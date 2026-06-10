use nexus_common::models::event::{Event, EventProcessorError, EventType, ParseResult};

use crate::events::handle;
use crate::events::retry::event::RetryEvent;
use crate::events::Moderation;
use crate::service::traits::TEventProcessor;
use nexus_common::db::PubkyConnector;
use nexus_common::models::homeserver::Homeserver;
use opentelemetry::metrics::Counter;
use opentelemetry::trace::{FutureExt, Span, TraceContextExt, Tracer};
use opentelemetry::{global, Context, KeyValue};
use pubky::Method;
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

/// OpenTelemetry meter name for all watcher metrics.
const METER_NAME: &str = "nexus.watcher";

/// Counter for events permanently rejected for exceeding a fetch size limit.
static REJECTED: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(METER_NAME)
        .u64_counter("watcher.fetch.rejected")
        .with_description("Event fetches rejected for exceeding a size limit")
        .build()
});

pub struct EventProcessor {
    pub homeserver: Homeserver,
    /// See [WatcherConfig::events_limit]
    pub limit: u32,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Arc<Moderation>,
    pub shutdown_rx: Receiver<bool>,
    /// See [WatcherConfig::max_file_size]
    pub max_file_size: u64,
}

#[async_trait::async_trait]
impl TEventProcessor for EventProcessor {
    fn get_homeserver_id(&self) -> PubkyId {
        self.homeserver.id.clone()
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let maybe_event_lines = self
            .poll_events()
            .await
            .inspect_err(|e| error!("Error polling events: {e:?}"))?;

        match maybe_event_lines {
            None => debug!("No new events"),
            Some(event_lines) => {
                info!("Processing {} event lines", event_lines.len());
                self.process_event_lines(event_lines).await?;
            }
        }

        Ok(())
    }
}

impl EventProcessor {
    /// Polls new events from the homeserver.
    ///
    /// It sends a GET request to the homeserver's events endpoint
    /// using the current cursor and a specified limit. It retrieves new event
    /// URIs in a newline-separated format, processes it into a vector of strings,
    /// and returns the result.
    #[tracing::instrument(name = "events.poll", skip_all, fields(homeserver = %self.homeserver.id))]
    async fn poll_events(&self) -> Result<Option<Vec<String>>, EventProcessorError> {
        debug!("Polling new events from homeserver");

        let response_text = {
            let pubky = PubkyConnector::get()?;
            let url = format!(
                "https://{}/events/?cursor={}&limit={}",
                self.homeserver.id, self.homeserver.cursor, self.limit
            );

            let response = pubky
                .client()
                .request(Method::GET, &url)
                .send()
                .await
                .map_err(|e| EventProcessorError::client_error(e.to_string()))?;

            response
                .text()
                .await
                .map_err(|e| EventProcessorError::client_error(e.to_string()))?
        };

        let lines: Vec<String> = response_text.trim().lines().map(String::from).collect();
        debug!("Homeserver response lines {:?}", lines);

        if lines.is_empty() || (lines.len() == 1 && lines[0].is_empty()) {
            return Ok(None);
        }

        Ok(Some(lines))
    }

    /// Processes a batch of event lines retrieved from the homeserver.
    ///
    /// This function iterates over a vector of event URIs, handling each line based on its content:
    /// - Lines starting with `cursor:` update the cursor for the homeserver and save it to the index.
    /// - Other lines are parsed into events and processed accordingly. If parsing fails, an error is logged.
    ///
    /// # Parameters
    /// - `lines`: A vector of strings representing event lines retrieved from the homeserver.
    #[tracing::instrument(name = "event_batch.process", skip_all, fields(batch.size = lines.len()))]
    pub async fn process_event_lines(&self, lines: Vec<String>) -> Result<(), EventProcessorError> {
        for line in &lines {
            let id = self.homeserver.id.clone();

            if *self.shutdown_rx.borrow() {
                debug!("Shutdown detected while processing HS {id}, exiting event processing loop");
                return Ok(());
            }

            if let Some(cursor) = line.strip_prefix("cursor: ") {
                info!("Received cursor for the next request: {cursor}");
                match Homeserver::try_from_cursor(id, cursor) {
                    Ok(hs) => hs.put_to_index().await?,
                    Err(e) => warn!("{e}"),
                }
            } else {
                match Event::parse_event(line, self.files_path.clone()) {
                    Err(e) => error!("{e}"),
                    Ok(ParseResult::Skipped) => {}
                    Ok(ParseResult::UnrecognizedUri {
                        event_type,
                        uri,
                        reason,
                    }) => {
                        if !self.try_handle_universal_tag(&event_type, &uri).await {
                            error!("Cannot parse event URI: {reason}");
                        }
                    }
                    Ok(ParseResult::Parsed(event)) => {
                        let tracer = global::tracer(self.tracer_name.clone());
                        let mut span = tracer.start(event.parsed_uri.resource.to_string());
                        span.set_attribute(KeyValue::new("event.uri", event.uri.clone()));
                        span.set_attribute(KeyValue::new(
                            "event.type",
                            event.event_type.to_string(),
                        ));
                        span.set_attribute(KeyValue::new(
                            "event.user_id",
                            event.parsed_uri.user_id.to_string(),
                        ));
                        span.set_attribute(KeyValue::new(
                            "event.resource_id",
                            event.parsed_uri.resource.id().unwrap_or("".to_string()),
                        ));
                        let cx = Context::new().with_span(span);
                        debug!("Processing event: {:?}", event);
                        self.handle_event(&event).with_context(cx).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Attempts to handle an unrecognized URI as a universal tag at an app-specific path.
    /// Returns `true` if the event was claimed (regardless of success/failure).
    async fn try_handle_universal_tag(&self, event_type: &EventType, uri: &str) -> bool {
        let result = crate::events::handlers::universal_tag::try_handle(event_type, uri).await;

        let Some(result) = result else {
            return false;
        };

        if let Err(e) = result {
            match e {
                EventProcessorError::InvalidEventLine(ref msg) => {
                    error!("Universal tag non-retryable: {msg}");
                }
                EventProcessorError::FetchSizeExceeded(size, limit) => {
                    // Deterministic: re-fetching produces the same result
                    warn!(size, limit, uri, "FetchSizeExceeded: permanently rejected");
                    REJECTED.add(1, &[KeyValue::new("reason", "size_exceeded")]);
                }
                _ => {
                    let index_key = format!("{event_type}:{uri}");
                    let retry_event = RetryEvent::new(e);
                    error!("{}, {}", retry_event.error_type, index_key);
                    if let Err(err) = retry_event.put_to_index(index_key).await {
                        error!("Failed to enqueue universal tag retry: {err}");
                    }
                }
            }
        }

        true
    }

    /// Processes an event and track the fail event it if necessary
    /// # Parameters:
    /// - `event`: The event to be processed
    #[tracing::instrument(
        name = "event.process",
        skip_all,
        fields(
            event.resource = %event.parsed_uri.resource,
            event.uri = %event.uri,
            event.r#type = %event.event_type,
            event.user_id = %event.parsed_uri.user_id,
            event.resource_id = event.parsed_uri.resource.id().unwrap_or_default(),
            homeserver = %self.homeserver.id,
            otel.status_code = tracing::field::Empty,
            otel.status_message = tracing::field::Empty,
        )
    )]
    async fn handle_event(&self, event: &Event) -> Result<(), EventProcessorError> {
        let span = tracing::Span::current();
        if let Err(e) = handle(event, self.moderation.clone(), self.max_file_size).await {
            span.record("otel.status_code", "ERROR");
            span.record("otel.status_message", tracing::field::display(&e));

            if let Some((index_key, retry_event)) = extract_retry_event_info(event, e) {
                error!("{}, {}", retry_event.error_type, index_key);
                if let Err(err) = retry_event.put_to_index(index_key).await {
                    error!("Failed to put event to retry index: {}", err);
                }
            }
        } else {
            span.record("otel.status_code", "OK");
        }
        Ok(())
    }
}

/// Returns `Some` if the error is retryable, otherwise logs and returns `None`.
fn extract_retry_event_info(
    event: &Event,
    error: EventProcessorError,
) -> Option<(String, RetryEvent)> {
    let retry_event = match error {
        EventProcessorError::InvalidEventLine(ref message) => {
            error!("{}", message);
            return None;
        }
        EventProcessorError::SpecValidation(ref reason) => {
            // Spec-validation failures are deterministic: re-running the same
            // payload produces the same error. Don't poison the retry queue.
            error!("SpecValidation: {}", reason);
            return None;
        }
        EventProcessorError::FetchSizeExceeded(size, limit) => {
            // Deterministic: re-fetching produces the same result
            warn!(
                size,
                limit,
                uri = %event.uri,
                "FetchSizeExceeded: permanently rejected"
            );
            REJECTED.add(1, &[KeyValue::new("reason", "size_exceeded")]);
            return None;
        }
        _ => RetryEvent::new(error),
    };

    // Generate a compress index to save in the cache
    let index = match RetryEvent::generate_index_key(&event.uri) {
        Some(retry_index) => retry_index,
        None => {
            return None;
        }
    };
    Some((format!("{}:{}", event.event_type, index), retry_event))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::models::event::ParseResult;
    use tempfile::TempDir;

    /// Build a syntactically-valid `Event` for classifier tests. The body of
    /// `extract_retry_event_info` only consults `event.uri` for the retry-eligible
    /// branch; the skip branches we test below short-circuit before reaching it.
    /// The returned `TempDir` must be kept alive for the duration of the test so
    /// the path stored on the `Event` remains valid.
    fn fixture_event() -> (Event, TempDir) {
        let tmp = tempfile::tempdir().expect("create temp files_path");
        let line = "PUT pubky://4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro/pub/pubky.app/posts/0034A0X7NJ52A";
        let event = match Event::parse_event(line, tmp.path().to_path_buf()).unwrap() {
            ParseResult::Parsed(event) => event,
            other => panic!("expected Parsed event, got {:?}", other),
        };
        (event, tmp)
    }

    #[test]
    fn test_extract_retry_event_info_skips_invalid_event_line() {
        // Regression guard: the pre-existing non-retryable branch must keep
        // returning `None` so retry-queue behavior doesn't change.
        let (event, _tmp) = fixture_event();
        let result = extract_retry_event_info(
            &event,
            EventProcessorError::InvalidEventLine("malformed".into()),
        );
        assert!(result.is_none(), "InvalidEventLine must skip retry");
    }

    #[test]
    fn test_extract_retry_event_info_skips_spec_validation() {
        // Spec-validation failures (e.g. unknown post kind, malformed
        // Collection envelope) are deterministic — re-running the same
        // payload produces the same error, so they must NOT enqueue a retry.
        // Without this, the v0.4.5 forwards-compat shim is theatre.
        let (event, _tmp) = fixture_event();
        let result = extract_retry_event_info(
            &event,
            EventProcessorError::SpecValidation("post kind is unknown".into()),
        );
        assert!(result.is_none(), "SpecValidation must skip retry");
    }

    #[test]
    fn test_extract_retry_event_info_skips_response_size_exceeded() {
        let (event, _tmp) = fixture_event();
        let result = extract_retry_event_info(
            &event,
            EventProcessorError::FetchSizeExceeded(100_000_000, 50_000_000),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_retry_event_info_retries_generic_errors() {
        // Counterpart: a non-classified (transient-looking) error still
        // enqueues a retry, so we don't accidentally drop recoverable failures.
        let (event, _tmp) = fixture_event();
        let result = extract_retry_event_info(
            &event,
            EventProcessorError::Generic("transient failure".into()),
        );
        assert!(result.is_some(), "Generic errors must enqueue a retry");
    }
}
