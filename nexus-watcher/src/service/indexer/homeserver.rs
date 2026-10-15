use super::{RunOutcome, TEventProcessor};
use crate::errors::EventProcessorError;
use crate::events::retry::RetryScheduler;
use crate::events::{
    format_error_body, read_stream_capped, Event, EventHandler, MAX_ERROR_BODY, MAX_EVENTS_BODY,
};
use nexus_common::db::kv::RedisError;
use nexus_common::db::{
    fetch_row_from_graph, queries, GraphResult, PubkyClientError, PubkyConnector,
};
use nexus_common::models::error::ModelError;
use nexus_common::models::homeserver::Homeserver;
use opentelemetry::metrics::Counter;
use opentelemetry::{global, KeyValue};
use pubky::Method;
use pubky_app_specs::PubkyId;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::watch::Receiver;
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace, warn};

/// Counter for events permanently rejected for exceeding a fetch size limit.
static REJECTED: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(super::METER_NAME)
        .u64_counter("watcher.fetch.rejected")
        .with_description("Event fetches rejected for exceeding a size limit")
        .build()
});

/// Counter for cursor lines the Primary HS sent that could not be applied.
///
/// A non-empty count means the HS returned a cursor we cannot parse, that would
/// move the stored cursor backward, or that advances without delivering events.
/// The cursor is not advanced and the same position is fetched on the next poll.
/// A sustained non-zero rate for one HS indicates it needs operator attention.
static INVALID_CURSOR_PRIMARY_HS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(super::METER_NAME)
        .u64_counter("watcher.primary_hs.cursor.invalid")
        .with_description("Cursor lines from the Primary HS that could not be applied")
        .build()
});

/// Counter for Primary HS batches that carried events but did not advance the cursor.
///
/// Distinct from [`INVALID_CURSOR_PRIMARY_HS`]: the cursor parses and does not
/// rewind, it simply holds at the position the batch was fetched with. Applying
/// it would make the next poll re-request and re-process the identical batch, so
/// the batch is skipped instead. Any non-zero count means the HS stream cannot
/// make progress and needs operator attention.
///
/// Only successful (2xx) responses reach the batch checks: a non-2xx poll fails
/// the run in [`event_lines_from_response`] with its status and body instead,
/// so an HS outage is recorded as a failed poll, not as a stalled cursor.
static STALLED_CURSOR_PRIMARY_HS: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(super::METER_NAME)
        .u64_counter("watcher.primary_hs.cursor.stalled")
        .with_description("Primary HS batches whose cursor did not advance past the requested one")
        .build()
});

/// A user's `HOSTED_BY` mapping, classified relative to a processor's HS.
///
/// The `stale` flag is only carried where it is meaningful: a stale mapping means
/// the user's published HS has diverged from the stored one (see
/// [`set_user_homeserver_stale`](nexus_common::db::queries::put::set_user_homeserver_stale)).
#[derive(Clone)]
pub enum HsMapping {
    /// The user has no `HOSTED_BY` edge yet.
    Unbound,
    /// The user is mapped to this processor's HS.
    Current { stale: bool },
    /// The user is mapped to a different HS.
    Other { hs_id: String },
}

/// A homeserver response, split into its event lines and the cursor closing the batch.
///
/// The homeserver sends the cursor line last, but that is positional convention
/// rather than something we can rely on from an untrusted peer: the split is
/// order-independent, and if a batch carries several cursor lines the last one
/// wins, being the position the homeserver ends the batch at.
struct EventBatch<'a> {
    event_lines: Vec<&'a str>,
    /// Raw, still unparsed value of the batch's cursor line, if it carried one.
    cursor: Option<&'a str>,
}

impl<'a> EventBatch<'a> {
    fn split(lines: &'a [String]) -> Self {
        let mut event_lines = Vec::with_capacity(lines.len());
        let mut cursor = None;

        for line in lines {
            match line.strip_prefix("cursor: ") {
                Some(value) => cursor = Some(value),
                None => event_lines.push(line.as_str()),
            }
        }

        Self {
            event_lines,
            cursor,
        }
    }

    /// Whether the batch carried anything other than its cursor line.
    ///
    /// Deliberately counts *every* non-cursor line, including ones that later
    /// parse as skipped or unrecognized: those still legitimately advance the
    /// cursor, so narrowing this to "lines that reached a handler" would let a
    /// non-advancing cursor through and re-open the replay loop.
    fn has_events(&self) -> bool {
        !self.event_lines.is_empty()
    }
}

/// Why a batch's closing cursor was not applied. Each variant has its own metric.
enum CursorRejection {
    /// Unparseable, would rewind the stored cursor, or advances without events.
    Invalid,
    /// The batch carried events, yet the checkpoint cannot move past them: the
    /// cursor holds at the position the batch was fetched with, or the batch
    /// carries no cursor line at all.
    Stalled,
}

/// Event processor for the primary homeserver
pub struct HsEventProcessor {
    /// The primary HS endpoint this processor fetches events from
    pub homeserver: Homeserver,

    /// See [WatcherConfig::events_limit]
    pub limit: u16,
    pub event_handler: Arc<dyn EventHandler>,
    pub shutdown_rx: Receiver<bool>,

    /// Scheduler used to enqueue failed events onto the retry queue
    pub retry_scheduler: Arc<RetryScheduler>,

    /// Per-run cache of users' `HOSTED_BY` mappings. For a given user's events in
    /// the events list, only the 1st one results in a graph lookup, the rest read from this cache.
    ///
    /// Entries are deliberately never refreshed within a run: once a user's mapping
    /// is resolved, the same decision is reused for the rest of the batch even if the
    /// resolver realigns the underlying edge mid-run. The cache is dropped when the run ends.
    pub hs_mapping_cache: Mutex<HashMap<String, HsMapping>>,
}

#[async_trait::async_trait]
impl TEventProcessor for HsEventProcessor {
    fn event_handler(&self) -> &Arc<dyn EventHandler> {
        &self.event_handler
    }

    fn instance_name(&self) -> &'static str {
        "HsEventProcessor"
    }

    fn retry_scheduler(&self) -> Option<&Arc<RetryScheduler>> {
        Some(&self.retry_scheduler)
    }

    fn homeserver_id(&self) -> Option<&str> {
        Some(self.homeserver.id.as_ref())
    }

    /// Skips events from users that are not actively bound to this homeserver.
    ///
    /// Before an event is processed we inspect the user's `HOSTED_BY` edge:
    /// - No edge, or a non-stale edge to this processor's homeserver: process.
    /// - A stale edge to this homeserver (the user's published homeserver has
    ///   diverged): log a warning and skip until the resolver realigns it.
    /// - An edge to a different homeserver: log a warning and skip.
    async fn should_process_event(&self, event: &Event) -> Result<bool, EventProcessorError> {
        let user_id = event.parsed_uri.user_id();

        match self.user_hs_mapping(user_id).await? {
            // No mapping yet (graceful fallback) or actively bound here: process.
            HsMapping::Unbound | HsMapping::Current { stale: false } => Ok(true),

            // Bound here but the mapping is stale: skip until the resolver realigns it.
            HsMapping::Current { stale: true } => {
                warn!(
                    event.uri = %event.uri,
                    user_id = %user_id,
                    "User's homeserver mapping is stale; skipping event"
                );
                Ok(false)
            }

            // Bound to a different homeserver: skip.
            HsMapping::Other { hs_id } => {
                warn!(
                    event.uri = %event.uri,
                    user_id = %user_id,
                    user_homeserver = %hs_id,
                    "User is hosted on a different homeserver; skipping event"
                );
                Ok(false)
            }
        }
    }

    async fn run_internal(self: Arc<Self>) -> Result<RunOutcome, EventProcessorError> {
        let maybe_event_lines = self
            .poll_events()
            .await
            .inspect_err(|e| error!(error = ?e, "Error polling events"))?;

        match maybe_event_lines {
            None => debug!("No new events"),
            Some(event_lines) => {
                info!(event_lines = event_lines.len(), "Processing event lines");
                self.process_event_lines(event_lines).await?;
            }
        }

        Ok(RunOutcome::Completed)
    }
}

impl HsEventProcessor {
    /// Resolves and caches a user's `HOSTED_BY` mapping relative to this processor's HS
    async fn user_hs_mapping(&self, user_id: &PubkyId) -> GraphResult<HsMapping> {
        if let Some(hs_mapping) = self.hs_mapping_cache.lock().await.get(user_id.as_ref()) {
            return Ok(hs_mapping.clone());
        }

        let query = queries::get::get_user_homeserver(user_id.as_ref());
        let mapping = match fetch_row_from_graph(query).await? {
            None => HsMapping::Unbound,
            Some(row) => {
                let hs_id: String = row.get("homeserver_id")?;
                let stale: bool = row.get("stale")?;

                if hs_id.as_str() == self.homeserver.id.as_ref() {
                    HsMapping::Current { stale }
                } else {
                    HsMapping::Other { hs_id }
                }
            }
        };

        self.hs_mapping_cache
            .lock()
            .await
            .insert(user_id.as_ref().to_string(), mapping.clone());

        Ok(mapping)
    }

    /// Polls new events from the homeserver.
    ///
    /// It sends a GET request to the homeserver's events endpoint
    /// using the current cursor and a specified limit. It retrieves new event
    /// URIs in a newline-separated format, processes it into a vector of strings,
    /// and returns the result.
    #[tracing::instrument(name = "events.poll", skip_all)]
    async fn poll_events(&self) -> Result<Option<Vec<String>>, EventProcessorError> {
        debug!(cursor = %self.homeserver.cursor, "Polling events");

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

        event_lines_from_response(&self.homeserver.id, response).await
    }

    /// Processes a batch of event lines retrieved from the homeserver.
    ///
    /// The batch's cursor is validated *before* any handler runs and persisted
    /// only once the whole batch has been processed. A cursor the homeserver
    /// should not have sent — or failed to send at all — therefore costs
    /// nothing: the batch is skipped whole, the stored cursor stays put, and the
    /// next poll re-fetches from the same position instead of repeating side
    /// effects the cursor would never record.
    ///
    /// Per-event retry logic:
    /// - On error that should not be retried right now: stops the batch, cursor is not saved, next tick replays from same position
    /// - On MissingDependency: stores event in retry queue, continues processing
    /// - On 404 (blob not found): skips indexing, continues processing
    /// - On InvalidEventLine/SkipIndexing: logs and continues
    ///
    /// # Parameters
    /// - `lines`: A vector of strings representing event lines retrieved from the homeserver.
    #[tracing::instrument(name = "event_batch.process", skip_all, fields(batch.size = lines.len()))]
    pub async fn process_event_lines(&self, lines: Vec<String>) -> Result<(), EventProcessorError> {
        let batch = EventBatch::split(&lines);

        let next_cursor = match batch.cursor {
            Some(raw_cursor) => {
                match self
                    .resolve_batch_cursor(raw_cursor, batch.has_events())
                    .await?
                {
                    Some(homeserver) => Some(homeserver),
                    // Rejected: skip the batch entirely. Running the handlers
                    // would only queue up work the next poll has to repeat,
                    // since the cursor stays where it is either way.
                    None => return Ok(()),
                }
            }

            // Events with no cursor line to close them leave the checkpoint
            // exactly where a non-advancing cursor would, so the next poll
            // re-fetches and re-processes this same batch — the same stall,
            // reached by omission rather than by repetition. A homeserver
            // sending events always sends the cursor that closes them, so this
            // is a truncated or malformed response (error pages never get
            // here: `poll_events` fails the run on a non-2xx).
            None if batch.has_events() => {
                self.reject_batch(
                    &CursorRejection::Stalled,
                    "batch carried events but no cursor line to advance the checkpoint",
                );
                return Ok(());
            }

            None => None,
        };

        for line in &batch.event_lines {
            if *self.shutdown_rx.borrow() {
                debug!("Shutdown detected; exiting event processing loop");
                // The cursor is deliberately left unpersisted: the batch is only
                // partly processed, so the next run has to replay it.
                return Ok(());
            }

            self.process_event_line(line).await?;
        }

        if let Some(homeserver) = next_cursor {
            homeserver.put_to_index().await?;
        }

        Ok(())
    }

    /// Resolves the cursor line closing a batch into the homeserver record to
    /// persist once the batch has been processed.
    ///
    /// `batch_had_events` reports whether the batch carried anything besides the
    /// cursor line itself, which decides how strict the check is.
    ///
    /// Returns `Ok(None)` when the cursor must not be applied, so the caller
    /// skips the batch. Redis failures propagate instead of being charged to the
    /// homeserver as bad input, so the run is recorded as failed and retried on
    /// the next scheduled execution.
    async fn resolve_batch_cursor(
        &self,
        raw_cursor: &str,
        batch_had_events: bool,
    ) -> Result<Option<Homeserver>, EventProcessorError> {
        info!("Received cursor for the next request: {raw_cursor}");

        let homeserver = match Homeserver::try_from_cursor(self.homeserver.id.clone(), raw_cursor)
            .await
        {
            Ok(homeserver) => homeserver,
            // Only the monotonicity guard raises `InvalidInput`; every other
            // Redis error is our own infrastructure failing, not bad HS input,
            // and must fail the run rather than silently skip a batch.
            Err(ModelError::KvOperationFailed(e)) if !matches!(e, RedisError::InvalidInput(_)) => {
                return Err(ModelError::KvOperationFailed(e).into());
            }
            // Unparseable, or it would rewind the stored cursor.
            Err(e) => {
                let reason = format!("cursor '{raw_cursor}' cannot be applied: {e}");
                self.reject_batch(&CursorRejection::Invalid, &reason);
                return Ok(None);
            }
        };

        // A batch carrying events has to move the cursor strictly forward. The
        // fetch is cursor-exclusive, so those events sit above the cursor the
        // batch was requested with (`self.homeserver.cursor`, read from Redis
        // when this processor was built and unchanged since — one poll per run).
        // A cursor merely holding at that position would make the next poll
        // re-request and re-process this exact batch, indefinitely.
        if batch_had_events && homeserver.cursor <= self.homeserver.cursor {
            let reason = format!(
                "batch carried events but cursor '{raw_cursor}' does not advance past the requested {}",
                self.homeserver.cursor
            );
            self.reject_batch(&CursorRejection::Stalled, &reason);
            return Ok(None);
        }

        // An idle response cannot prove that any intervening events were
        // delivered, so it may repeat the requested cursor but never advance it.
        if !batch_had_events && homeserver.cursor != self.homeserver.cursor {
            let reason = format!(
                "batch carried no events but cursor '{raw_cursor}' differs from the requested {}",
                self.homeserver.cursor
            );
            self.reject_batch(&CursorRejection::Invalid, &reason);
            return Ok(None);
        }

        Ok(Some(homeserver))
    }

    /// Records a skipped batch on the metric matching `rejection`, and warns.
    fn reject_batch(&self, rejection: &CursorRejection, reason: &str) {
        let counter = match rejection {
            CursorRejection::Invalid => &INVALID_CURSOR_PRIMARY_HS,
            CursorRejection::Stalled => &STALLED_CURSOR_PRIMARY_HS,
        };
        counter.add(1, &[KeyValue::new("hs_id", self.homeserver.id.to_string())]);

        warn!(hs_id = %self.homeserver.id, "Skipping batch: {reason}");
    }
}

/// Turns a `/events/` response into the event lines it carries, or `None` for
/// an empty body.
///
/// The status is checked before the body is read: an error page has lines and
/// no `cursor:` line, so parsed as events it would be rejected as a stalled
/// cursor. A non-2xx instead fails the poll with the status and a capped body,
/// the way [`handle_put_event`](crate::events::handle_put_event) fails a fetch.
async fn event_lines_from_response(
    hs_id: &PubkyId,
    response: reqwest::Response,
) -> Result<Option<Vec<String>>, EventProcessorError> {
    let status = response.status();
    if !status.is_success() {
        let (body, _) = read_stream_capped(response.bytes_stream(), MAX_ERROR_BODY)
            .await
            .unwrap_or_default();
        let body = format_error_body(&body, MAX_ERROR_BODY);

        let message = format!("Polling events failed {hs_id}: HTTP {status} - {body}");
        return Err(client_error_for_status(status, message));
    }

    let (buf, exceeded) = read_stream_capped(response.bytes_stream(), MAX_EVENTS_BODY)
        .await
        .map_err(|e| EventProcessorError::client_error(e.to_string()))?;
    if exceeded {
        REJECTED.add(1, &[KeyValue::new("reason", "size_exceeded")]);

        return Err(EventProcessorError::FetchSizeExceeded(
            buf.len() as u64,
            MAX_EVENTS_BODY as u64,
        ));
    }

    let lines: Vec<String> = String::from_utf8_lossy(&buf)
        .trim()
        .lines()
        .map(String::from)
        .collect();
    trace!(?lines, "Homeserver response lines");

    Ok((!lines.is_empty()).then_some(lines))
}

/// Classifies a non-2xx status the same way
/// [`PubkyClientError::from(pubky::Error)`](PubkyClientError) classifies the
/// SDK's own errors, so a raw-client poll surfaces as the same variants as the
/// SDK-driven fetches do.
fn client_error_for_status(status: StatusCode, message: String) -> EventProcessorError {
    let error = match status {
        StatusCode::NOT_FOUND => PubkyClientError::NotFound404 { message },
        StatusCode::TOO_MANY_REQUESTS => PubkyClientError::TooManyRequests429 { message },
        s if s.is_server_error() => PubkyClientError::ServerError5xx { message },
        _ => PubkyClientError::RequestFailed { message },
    };
    error.into()
}

#[cfg(test)]
mod tests {
    use super::{event_lines_from_response, EventBatch};
    use crate::errors::EventProcessorError;
    use crate::events::{MAX_ERROR_BODY, MAX_EVENTS_BODY};
    use nexus_common::db::PubkyClientError;
    use pubky_app_specs::PubkyId;
    use std::sync::Arc;

    const HS_ID: &str = "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy";

    fn hs_id() -> PubkyId {
        PubkyId::try_from(HS_ID).expect("valid test Pubky ID")
    }

    fn response(status: u16, body: impl Into<reqwest::Body>) -> reqwest::Response {
        let response = http::Response::builder()
            .status(status)
            .body(body.into())
            .expect("valid response");
        reqwest::Response::from(response)
    }

    /// Unwraps the `PubkyClientError` a failed poll is classified as.
    fn client_error(error: EventProcessorError) -> Arc<PubkyClientError> {
        match error {
            EventProcessorError::PubkyClientError(e) => e,
            other => panic!("expected PubkyClientError, got {other:?}"),
        }
    }

    fn lines(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|l| l.to_string()).collect()
    }

    // --- event_lines_from_response ---

    /// The incident shape: a proxy error page. Before the status check this
    /// read as "events with no cursor line" and was counted as a stalled cursor.
    #[tokio::test]
    async fn error_page_fails_the_poll_with_status_and_body() {
        let html = "<html>\n<head><title>502 Bad Gateway</title></head>\n<body>bad gateway</body>\n</html>";

        let error = event_lines_from_response(&hs_id(), response(502, html))
            .await
            .unwrap_err();

        match client_error(error).as_ref() {
            PubkyClientError::ServerError5xx { message } => {
                assert!(message.contains(HS_ID), "names the HS: {message}");
                assert!(
                    message.contains("HTTP 502"),
                    "carries the status: {message}"
                );
                assert!(
                    message.contains("502 Bad Gateway"),
                    "carries the body: {message}"
                );
            }
            other => panic!("expected ServerError5xx, got {other:?}"),
        }
    }

    /// Each status lands on the same variant the SDK would map it to, so a 429
    /// is recognisable to callers that back off on rate limiting.
    #[tokio::test]
    async fn non_2xx_statuses_are_classified() {
        for status in [404u16, 429, 503, 400] {
            let error = event_lines_from_response(&hs_id(), response(status, "nope"))
                .await
                .unwrap_err();
            assert_eq!(error.is_too_many_requests(), status == 429);

            let error = client_error(error);

            let classified_right = matches!(
                (status, error.as_ref()),
                (404, PubkyClientError::NotFound404 { .. })
                    | (429, PubkyClientError::TooManyRequests429 { .. })
                    | (503, PubkyClientError::ServerError5xx { .. })
                    | (400, PubkyClientError::RequestFailed { .. })
            );
            assert!(classified_right, "HTTP {status} classified as {error:?}");
        }
    }

    /// An oversized error body is truncated into the message, not rejected as
    /// a size violation: the size cap exists for event batches.
    #[tokio::test]
    async fn error_body_is_capped() {
        let body = "x".repeat(MAX_ERROR_BODY * 4);

        let error = event_lines_from_response(&hs_id(), response(500, body))
            .await
            .unwrap_err();

        match client_error(error).as_ref() {
            PubkyClientError::ServerError5xx { message } => {
                assert!(message.ends_with("… (truncated)"), "{message}");
                assert!(message.len() < MAX_ERROR_BODY * 2, "{}", message.len());
            }
            other => panic!("expected ServerError5xx, got {other:?}"),
        }
    }

    /// A non-2xx response whose body cannot be read still fails on the status.
    #[tokio::test]
    async fn unreadable_error_body_still_fails_on_status() {
        let body = reqwest::Body::wrap_stream(futures::stream::iter(vec![Err::<bytes::Bytes, _>(
            std::io::Error::other("connection reset"),
        )]));

        let error = event_lines_from_response(&hs_id(), response(502, body))
            .await
            .unwrap_err();

        assert!(matches!(
            client_error(error).as_ref(),
            PubkyClientError::ServerError5xx { .. }
        ));
    }

    /// The happy path is unchanged: a 2xx body is split into lines as before.
    #[tokio::test]
    async fn success_body_is_split_into_lines() {
        let body = "PUT pubky://a/pub/x\nDEL pubky://b/pub/y\ncursor: 42\n";

        let lines = event_lines_from_response(&hs_id(), response(200, body))
            .await
            .unwrap();

        assert_eq!(
            lines,
            Some(vec![
                "PUT pubky://a/pub/x".to_string(),
                "DEL pubky://b/pub/y".to_string(),
                "cursor: 42".to_string(),
            ])
        );
    }

    /// An idle poll answers 2xx with nothing in it.
    #[tokio::test]
    async fn empty_success_body_is_no_events() {
        let lines = event_lines_from_response(&hs_id(), response(200, ""))
            .await
            .unwrap();

        assert_eq!(lines, None);
    }

    /// A 2xx body over the events cap is still a size violation.
    #[tokio::test]
    async fn oversized_success_body_is_rejected() {
        let body = "x".repeat(MAX_EVENTS_BODY + 1);

        let error = event_lines_from_response(&hs_id(), response(200, body))
            .await
            .unwrap_err();

        assert!(matches!(error, EventProcessorError::FetchSizeExceeded(..)));
    }

    // --- EventBatch::split ---

    /// The usual shape: events followed by the cursor that closes the batch.
    #[test]
    fn splits_events_from_the_trailing_cursor() {
        let lines = lines(&["PUT pubky://a/pub/x", "DEL pubky://b/pub/y", "cursor: 42"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(
            batch.event_lines,
            vec!["PUT pubky://a/pub/x", "DEL pubky://b/pub/y"]
        );
        assert_eq!(batch.cursor, Some("42"));
        assert!(batch.has_events());
    }

    /// An idle poll: a cursor and nothing else, which must not read as "had events".
    #[test]
    fn cursor_only_batch_has_no_events() {
        let lines = lines(&["cursor: 42"]);
        let batch = EventBatch::split(&lines);

        assert!(batch.event_lines.is_empty());
        assert_eq!(batch.cursor, Some("42"));
        assert!(!batch.has_events());
    }

    /// The cursor is found wherever it sits, not only in last position.
    #[test]
    fn cursor_is_found_out_of_position() {
        let lines = lines(&["cursor: 42", "PUT pubky://a/pub/x"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(batch.event_lines, vec!["PUT pubky://a/pub/x"]);
        assert_eq!(batch.cursor, Some("42"));
    }

    /// Several cursor lines: the last one wins, being where the batch ends.
    #[test]
    fn last_cursor_line_wins() {
        let lines = lines(&["cursor: 41", "PUT pubky://a/pub/x", "cursor: 42"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(batch.event_lines, vec!["PUT pubky://a/pub/x"]);
        assert_eq!(batch.cursor, Some("42"));
    }

    /// A batch with no cursor line at all, which the caller rejects as stalled
    /// when it carries events: there is nothing to move the checkpoint with.
    #[test]
    fn batch_without_cursor_line() {
        let lines = lines(&["PUT pubky://a/pub/x"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(batch.event_lines, vec!["PUT pubky://a/pub/x"]);
        assert_eq!(batch.cursor, None);
        assert!(batch.has_events());
    }

    /// Lines that will later parse as skipped or unrecognized still count as
    /// events: they legitimately advance the cursor, so a batch made only of
    /// them must not be treated as an idle poll.
    #[test]
    fn unrecognized_lines_count_as_events() {
        let lines = lines(&["garbage", "cursor: 42"]);
        let batch = EventBatch::split(&lines);

        assert_eq!(batch.event_lines, vec!["garbage"]);
        assert!(batch.has_events());
    }
}
