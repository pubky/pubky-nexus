use nexus_common::models::event::{Event, EventProcessorError};

use crate::events::handle;
use crate::events::retry::event::RetryEvent;
use crate::events::Moderation;
use crate::service::traits::TEventProcessor;
use nexus_common::db::PubkyConnector;
use nexus_common::models::homeserver::Homeserver;
use opentelemetry::trace::{FutureExt, Span, TraceContextExt, Tracer};
use opentelemetry::{global, Context, KeyValue};
use pubky::Method;
use pubky_app_specs::PubkyId;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tracing::{debug, error, info, warn};

pub struct EventProcessor {
    pub homeserver: Homeserver,
    /// See [WatcherConfig::events_limit]
    pub limit: u32,
    pub files_path: PathBuf,
    pub tracer_name: String,
    pub moderation: Arc<Moderation>,
    pub shutdown_rx: Receiver<bool>,
}

#[async_trait::async_trait]
impl TEventProcessor for EventProcessor {
    fn get_homeserver_id(&self) -> PubkyId {
        self.homeserver.id.clone()
    }

    async fn run_internal(self: Arc<Self>) -> Result<(), EventProcessorError> {
        let maybe_event_lines = {
            let tracer = global::tracer(self.tracer_name.clone());
            let span = tracer.start("Polling Events");
            let cx = Context::new().with_span(span);
            self.poll_events()
                .with_context(cx)
                .await
                .inspect_err(|e| error!("Error polling events: {e:?}"))?
        };

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
                let maybe_event = Event::parse_event(line, self.files_path.clone())
                    .inspect_err(|e| error!("{e}"))
                    .unwrap_or(None);

                if let Some(event) = maybe_event {
                    let tracer = global::tracer(self.tracer_name.clone());
                    let mut span = tracer.start(event.parsed_uri.resource.to_string());
                    span.set_attribute(KeyValue::new("event.uri", event.uri.clone()));
                    span.set_attribute(KeyValue::new("event.type", event.event_type.to_string()));
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

        Ok(())
    }

    /// Processes an event and track the fail event it if necessary
    /// # Parameters:
    /// - `event`: The event to be processed
    async fn handle_event(&self, event: &Event) -> Result<(), EventProcessorError> {
        if let Err(e) = handle(event, self.moderation.clone()).await {
            if let Some((index_key, retry_event)) = extract_retry_event_info(event, e) {
                error!("{}, {}", retry_event.error_type, index_key);
                if let Err(err) = retry_event.put_to_index(index_key).await {
                    error!("Failed to put event to retry index: {}", err);
                }
            }
        }
        Ok(())
    }
}

/// Extracts retry-related information from an event and its associated error
///
/// # Parameters
/// - `event`: Reference to the event for which retry information is being extracted
/// - `error`: Determines whether the event is eligible for a retry or should be discarded
fn extract_retry_event_info(
    event: &Event,
    error: EventProcessorError,
) -> Option<(String, RetryEvent)> {
    let retry_event = match error {
        EventProcessorError::InvalidEventLine(ref message) => {
            error!("{}", message);
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
