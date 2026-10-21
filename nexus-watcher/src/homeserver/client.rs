use std::future::Future;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use futures::{FutureExt, Stream, StreamExt};
use nexus_common::db::PubkyClientResult;
use nexus_common::models::user::PkdnsHomeserverResolver;
use nexus_common::WatcherConfig;
use opentelemetry::metrics::Counter;
use opentelemetry::{global, KeyValue};
use pubky::{EventCursor, Method, Pubky, PubkyResource, PublicKey};
use pubky_app_specs::PubkyId;
use tracing::{error, trace, warn};

use super::fetch::{
    fetch_capped, read_stream_capped, MAX_ERROR_BODY, MAX_EVENTS_BODY, MAX_RESOURCE_SIZE,
};
use super::{HsError, HsErrorKind, HsOp};
use crate::events::EventType;
use crate::METER_NAME;

/// Waits between the retries of a rate-limited `/events-stream` request.
const USER_EVENTS_429_BACKOFF_SECS: [u64; 3] = [1, 2, 3];

/// Counter for events permanently rejected for exceeding a fetch size limit.
static REJECTED: LazyLock<Counter<u64>> = LazyLock::new(|| {
    global::meter(METER_NAME)
        .u64_counter("watcher.fetch.rejected")
        .with_description("Event fetches rejected for exceeding a size limit")
        .build()
});

/// One event from a homeserver's per-user `/events-stream`: all the key-based
/// processor needs from it.
#[derive(Debug, Clone)]
pub struct HsStreamEvent {
    pub event_type: EventType,
    pub cursor: u64,
    owner: PublicKey,
    uri: String,
}

impl HsStreamEvent {
    pub fn new(event_type: EventType, resource: &PubkyResource, cursor: u64) -> Self {
        Self {
            event_type,
            cursor,
            owner: resource.owner.clone(),
            uri: resource.to_pubky_url(),
        }
    }

    /// Owner of [`Self::uri`], checked against the user whose stream the event is on.
    /// Both come from one resource, so they cannot disagree.
    pub fn owner(&self) -> &PublicKey {
        &self.owner
    }

    /// The resource's `pubky://` URI.
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

impl From<pubky::Event> for HsStreamEvent {
    fn from(event: pubky::Event) -> Self {
        Self::new(event.event_type.into(), &event.resource, event.cursor.id())
    }
}

/// Every HTTP call the watcher makes to a homeserver.
///
/// A failure is labelled with `hs`, the homeserver the caller polls, or for a resource or
/// blob with its owner's key.
///
/// It also resolves the homeserver a user publishes ([`PkdnsHomeserverResolver`]), for the
/// user ingestor and the periodic user-to-homeserver resolver.
#[async_trait::async_trait]
pub trait HomeserverClient: PkdnsHomeserverResolver + Send + Sync + 'static {
    /// Primary-style `GET /events/?cursor=&limit=`. Returns the raw lines, the
    /// cursor line included; an empty body has none.
    async fn poll_events(
        &self,
        hs: &PubkyId,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<String>, HsError>;

    /// Per-user `GET /events-stream?user=<pk>:<cursor>&limit=&path=/pub/`, not live.
    /// At most `limit` events are returned. A rate-limited request is retried.
    async fn user_events(
        &self,
        hs: &PubkyId,
        user: &PublicKey,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<HsStreamEvent>, HsError>;

    /// The JSON resource at `resource`, capped at [`MAX_RESOURCE_SIZE`], from the homeserver
    /// its owner publishes.
    async fn get_resource(&self, resource: &PubkyResource) -> Result<Vec<u8>, HsError>;

    /// The blob at `resource`, capped at the configured `max_file_size`, from the homeserver
    /// its owner publishes.
    async fn get_blob(&self, resource: &PubkyResource) -> Result<Vec<u8>, HsError>;
}

/// [`HomeserverClient`] over the Pubky SDK.
pub struct PubkyHomeserverClient {
    pubky: Arc<Pubky>,
    max_file_size: u64,
}

impl PubkyHomeserverClient {
    pub fn new(pubky: Arc<Pubky>, max_file_size: u64) -> Self {
        Self {
            pubky,
            max_file_size,
        }
    }

    /// Builds a client over `pubky`, with the file size limit of `config`.
    pub fn from_config(pubky: Arc<Pubky>, config: &WatcherConfig) -> Self {
        Self::new(pubky, config.max_file_size)
    }

    /// GETs `resource`, returning the response for the caller to read under its cap.
    ///
    /// The SDK reads a non-2xx body whole, before [`HsErrorKind::from_sdk`] keeps 4 KiB of
    /// it, so for resources and blobs that cap limits what is kept, not what is read.
    async fn get(&self, resource: &PubkyResource) -> Result<reqwest::Response, HsErrorKind> {
        self.pubky
            .public_storage()
            .get(resource)
            .await
            .map_err(HsErrorKind::from_sdk)
    }

    /// One `/events-stream` request, without retries.
    async fn fetch_user_events(
        &self,
        hs: &PubkyId,
        user: &PublicKey,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<HsStreamEvent>, HsErrorKind> {
        // We are building the stream without the live flag, so it performs an HTTP GET and closes.
        // See rustdoc of EventStreamBuilder::live()
        let mut stream = self
            .pubky
            .event_stream_for(&hs.to_public_key())
            .add_users(vec![(user, Some(EventCursor::new(cursor)))])
            .map_err(HsErrorKind::from_sdk)?
            .limit(limit)
            .path("/pub/")
            .subscribe()
            .await
            .map_err(HsErrorKind::from_sdk)
            .inspect_err(|e| error!(error = %e, "Failed to subscribe to event stream"))?;

        read_user_events(&mut stream, limit, hs, user).await
    }
}

/// Reads at most `limit` events of an `/events-stream` response. A homeserver that holds
/// the connection open once it has sent them is not waited for; one that has already sent
/// more than asked is logged, and the excess dropped. Pubky uses SSE framing regardless of
/// `EventStreamBuilder::live()`, so a failure after the response headers arrives as a
/// stream item, which fails the read as a transport error.
async fn read_user_events<S>(
    stream: &mut S,
    limit: u16,
    hs: &PubkyId,
    user: &PublicKey,
) -> Result<Vec<HsStreamEvent>, HsErrorKind>
where
    S: Stream<Item = pubky::Result<pubky::Event>> + Unpin,
{
    let limit = limit as usize;
    let mut events = Vec::with_capacity(limit);
    while events.len() < limit {
        let Some(result) = stream.next().await else {
            break;
        };
        let event = result.map_err(HsErrorKind::transport)?;
        events.push(HsStreamEvent::from(event));
    }

    // Polled once, never awaited: only an event that has already arrived counts.
    let sent_more =
        events.len() == limit && matches!(stream.next().now_or_never(), Some(Some(Ok(_))));
    if sent_more {
        error!(
            user_pk = %user.z32(),
            hs_pk = %hs,
            limit,
            "Event stream returned more than the requested limit; ignoring the excess"
        );
    }

    Ok(events)
}

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for PubkyHomeserverClient {
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>> {
        Ok(self
            .pubky
            .get_homeserver_of(user_pk)
            .await
            .map(PubkyId::from))
    }
}

#[async_trait::async_trait]
impl HomeserverClient for PubkyHomeserverClient {
    async fn poll_events(
        &self,
        hs: &PubkyId,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<String>, HsError> {
        let send = self
            .pubky
            .client()
            .request(Method::GET, &events_url(hs, cursor, limit))
            .send();
        poll_events_with(hs, send).await
    }

    async fn user_events(
        &self,
        hs: &PubkyId,
        user: &PublicKey,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<HsStreamEvent>, HsError> {
        user_events_with(hs, user, || self.fetch_user_events(hs, user, cursor, limit)).await
    }

    async fn get_resource(&self, resource: &PubkyResource) -> Result<Vec<u8>, HsError> {
        get_resource_with(resource, self.get(resource)).await
    }

    async fn get_blob(&self, resource: &PubkyResource) -> Result<Vec<u8>, HsError> {
        get_blob_with(resource, self.max_file_size, self.get(resource)).await
    }
}

/// Reads the lines of the `/events/` poll `send` makes, labelling a failure as `hs`'s poll.
async fn poll_events_with(
    hs: &PubkyId,
    send: impl Future<Output = Result<reqwest::Response, reqwest::Error>>,
) -> Result<Vec<String>, HsError> {
    let error = |kind| HsError::new(hs, HsOp::PollEvents, kind);
    let response = send
        .await
        .map_err(|e| error(HsErrorKind::from_reqwest(&e)))?;
    event_lines_from_response(response).await.map_err(error)
}

/// Fetches a user's events with `fetch`, retrying while rate limited, and labels a failure
/// as the `/events-stream` request's, which decides that it stops the homeserver's run.
async fn user_events_with<F, Fut>(
    hs: &PubkyId,
    user: &PublicKey,
    fetch: F,
) -> Result<Vec<HsStreamEvent>, HsError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Vec<HsStreamEvent>, HsErrorKind>>,
{
    retry_rate_limited(user, fetch)
        .await
        .map_err(|kind| HsError::new(hs, HsOp::UserEvents, kind))
}

/// Reads a resource fetch's body, at most [`MAX_RESOURCE_SIZE`], labelling a failure as a
/// resource's, which keeps it one event's failure.
async fn get_resource_with(
    resource: &PubkyResource,
    fetch: impl Future<Output = Result<reqwest::Response, HsErrorKind>>,
) -> Result<Vec<u8>, HsError> {
    read_capped(fetch, MAX_RESOURCE_SIZE as u64)
        .await
        .map_err(|kind| owner_error(resource, HsOp::GetResource, kind))
}

/// Reads a blob fetch's body, at most `max` bytes, labelling a failure as a blob's, which
/// keeps it one event's failure.
async fn get_blob_with(
    resource: &PubkyResource,
    max: u64,
    fetch: impl Future<Output = Result<reqwest::Response, HsErrorKind>>,
) -> Result<Vec<u8>, HsError> {
    read_capped(fetch, max)
        .await
        .map_err(|kind| owner_error(resource, HsOp::GetBlob, kind))
}

async fn read_capped(
    fetch: impl Future<Output = Result<reqwest::Response, HsErrorKind>>,
    max: u64,
) -> Result<Vec<u8>, HsErrorKind> {
    fetch_capped(fetch.await?, max).await
}

/// A failed resource or blob fetch, labelled with the owner's key: nothing is charged to a
/// homeserver yet, and resolving the one the owner publishes would cost a PKARR query on
/// every failure.
fn owner_error(resource: &PubkyResource, op: HsOp, kind: HsErrorKind) -> HsError {
    HsError::new(&PubkyId::from(resource.owner.clone()), op, kind)
}

/// Calls `fetch` and, while it is rate limited, again after each of
/// [`USER_EVENTS_429_BACKOFF_SECS`]. Returns the last 429 once they run out.
async fn retry_rate_limited<T, F, Fut>(user: &PublicKey, mut fetch: F) -> Result<T, HsErrorKind>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, HsErrorKind>>,
{
    let mut backoffs = USER_EVENTS_429_BACKOFF_SECS.into_iter();
    loop {
        let kind = match fetch().await {
            Ok(value) => return Ok(value),
            Err(kind) => kind,
        };
        let (HsErrorKind::RateLimited(_), Some(backoff_secs)) = (&kind, backoffs.next()) else {
            return Err(kind);
        };

        warn!(
            user_id = %user.z32(), retry_after_secs = backoff_secs,
            "Homeserver rate-limited user event fetch; retrying",
        );
        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
    }
}

/// The primary-style `/events/` URL of `hs`, from `cursor` on, for at most `limit` events.
fn events_url(hs: &PubkyId, cursor: u64, limit: u16) -> String {
    format!("https://{hs}/events/?cursor={cursor}&limit={limit}")
}

/// Turns a `/events/` response into the lines it carries.
///
/// The status is checked before the body is read: an error page has lines and
/// no `cursor:` line, so parsed as events it would be rejected as a stalled
/// cursor. A non-2xx instead fails the poll with the status and a capped body.
async fn event_lines_from_response(
    response: reqwest::Response,
) -> Result<Vec<String>, HsErrorKind> {
    let status = response.status();
    if !status.is_success() {
        let (body, _) = read_stream_capped(response.bytes_stream(), MAX_ERROR_BODY)
            .await
            .unwrap_or_default();
        return Err(HsErrorKind::from_status(status, &body));
    }

    let (buf, exceeded) = read_stream_capped(response.bytes_stream(), MAX_EVENTS_BODY)
        .await
        .map_err(|e| HsErrorKind::from_reqwest(&e))?;
    if exceeded {
        REJECTED.add(1, &[KeyValue::new("reason", "size_exceeded")]);

        return Err(HsErrorKind::TooLarge {
            got: buf.len() as u64,
            max: MAX_EVENTS_BODY as u64,
        });
    }

    let lines: Vec<String> = String::from_utf8_lossy(&buf)
        .trim()
        .lines()
        .map(String::from)
        .collect();
    trace!(?lines, "Homeserver response lines");

    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::super::error::transport_error;
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::Instant;

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

    // --- events_url ---

    #[test]
    fn events_url_carries_the_cursor_and_limit() {
        assert_eq!(
            events_url(&hs_id(), 42, 17),
            format!("https://{HS_ID}/events/?cursor=42&limit=17")
        );
    }

    // --- poll_events_with ---

    /// The incident shape: a proxy error page. Before the status check this
    /// read as "events with no cursor line" and was counted as a stalled cursor.
    #[tokio::test]
    async fn error_page_fails_the_poll_with_status_and_body() {
        let html = "<html>\n<head><title>502 Bad Gateway</title></head>\n<body>bad gateway</body>\n</html>";

        let error = poll_events_with(&hs_id(), async { Ok(response(502, html)) })
            .await
            .unwrap_err();

        assert_eq!(error.hs, HS_ID);
        assert_eq!(error.op, HsOp::PollEvents);
        assert!(
            matches!(error.kind, HsErrorKind::ServerError(502, _)),
            "{error:?}"
        );
        let message = error.to_string();
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

    /// A poll that gets no response is labelled as the homeserver's poll too, as a
    /// transport failure.
    #[tokio::test]
    async fn unanswered_poll_is_a_transport_failure_of_the_homeserver() {
        let error = poll_events_with(&hs_id(), async { Err(transport_error().await) })
            .await
            .unwrap_err();

        assert_eq!(error.hs, HS_ID);
        assert_eq!(error.op, HsOp::PollEvents);
        assert!(matches!(error.kind, HsErrorKind::Transport(_)), "{error:?}");
    }

    // --- event_lines_from_response ---

    /// Each status lands on its own kind, so a 429 is recognisable to callers
    /// that back off on rate limiting.
    #[tokio::test]
    async fn non_2xx_statuses_are_classified() {
        for status in [404u16, 429, 503, 400] {
            let kind = event_lines_from_response(response(status, "nope"))
                .await
                .unwrap_err();

            let classified_right = matches!(
                (status, &kind),
                (404, HsErrorKind::NotFound(_))
                    | (429, HsErrorKind::RateLimited(_))
                    | (503, HsErrorKind::ServerError(503, _))
                    | (400, HsErrorKind::Rejected(400, _))
            );
            assert!(classified_right, "HTTP {status} classified as {kind:?}");
        }
    }

    /// An oversized error body is truncated into the message, not rejected as
    /// a size violation: the size cap exists for event batches.
    #[tokio::test]
    async fn error_body_is_capped() {
        let body = "x".repeat(MAX_ERROR_BODY * 4);

        let kind = event_lines_from_response(response(500, body))
            .await
            .unwrap_err();

        let message = kind.to_string();
        assert!(matches!(kind, HsErrorKind::ServerError(500, _)), "{kind:?}");
        assert!(message.ends_with("… (truncated)"), "{message}");
        assert!(message.len() < MAX_ERROR_BODY * 2, "{}", message.len());
    }

    /// A non-2xx response whose body cannot be read still fails on the status.
    #[tokio::test]
    async fn unreadable_error_body_still_fails_on_status() {
        let body = reqwest::Body::wrap_stream(futures::stream::iter(vec![Err::<bytes::Bytes, _>(
            std::io::Error::other("connection reset"),
        )]));

        let kind = event_lines_from_response(response(502, body))
            .await
            .unwrap_err();

        assert!(matches!(kind, HsErrorKind::ServerError(502, _)), "{kind:?}");
    }

    /// The happy path is unchanged: a 2xx body is split into lines as before.
    #[tokio::test]
    async fn success_body_is_split_into_lines() {
        let body = "PUT pubky://a/pub/x\nDEL pubky://b/pub/y\ncursor: 42\n";

        let lines = event_lines_from_response(response(200, body))
            .await
            .unwrap();

        assert_eq!(
            lines,
            vec![
                "PUT pubky://a/pub/x".to_string(),
                "DEL pubky://b/pub/y".to_string(),
                "cursor: 42".to_string(),
            ]
        );
    }

    /// An idle poll answers 2xx with nothing in it.
    #[tokio::test]
    async fn empty_success_body_is_no_events() {
        let lines = event_lines_from_response(response(200, "")).await.unwrap();

        assert!(lines.is_empty());
    }

    /// A 2xx body over the events cap is still a size violation.
    #[tokio::test]
    async fn oversized_success_body_is_rejected() {
        let body = "x".repeat(MAX_EVENTS_BODY + 1);

        let kind = event_lines_from_response(response(200, body))
            .await
            .unwrap_err();

        assert!(matches!(kind, HsErrorKind::TooLarge { .. }), "{kind:?}");
    }

    // --- read_user_events ---

    fn stream_event(cursor: u64) -> pubky::Result<pubky::Event> {
        let owner = pubky::Keypair::random().public_key();
        Ok(pubky::Event {
            event_type: pubky::EventType::Delete,
            resource: PubkyResource::new(owner, "/pub/pubky.app/profile.json").expect("valid path"),
            cursor: EventCursor::new(cursor),
        })
    }

    /// The read ends once it holds exactly `limit` events: a homeserver that then holds the
    /// connection open is neither waited for nor logged. Time is paused, so a read that
    /// waits fails at once.
    #[tokio::test(start_paused = true)]
    #[tracing_test::traced_test]
    async fn user_events_read_ends_at_the_limit_without_waiting_for_more() {
        let user = pubky::Keypair::random().public_key();
        let mut stream = futures::stream::iter([stream_event(1), stream_event(2)])
            .chain(futures::stream::pending());

        let events = tokio::time::timeout(
            Duration::from_secs(1),
            read_user_events(&mut stream, 2, &hs_id(), &user),
        )
        .await
        .expect("the read must end at the limit")
        .unwrap();

        let cursors: Vec<u64> = events.iter().map(|event| event.cursor).collect();
        assert_eq!(cursors, [1, 2]);
        assert!(!logs_contain("more than the requested limit"));
    }

    /// A full page is normal: a homeserver that sends exactly `limit` events and ends the
    /// stream is not logged.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn user_events_exactly_at_the_limit_are_not_logged() {
        let user = pubky::Keypair::random().public_key();
        let mut stream = futures::stream::iter([stream_event(1), stream_event(2)]);

        let events = read_user_events(&mut stream, 2, &hs_id(), &user)
            .await
            .unwrap();

        assert_eq!(events.len(), 2);
        assert!(!logs_contain("more than the requested limit"));
    }

    /// A homeserver that has already sent more than the `limit` asked for is logged, and
    /// only the events asked for are returned.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn user_events_past_the_limit_are_logged_and_dropped() {
        let user = pubky::Keypair::random().public_key();
        let mut stream = futures::stream::iter([stream_event(1), stream_event(2), stream_event(3)]);

        let events = read_user_events(&mut stream, 2, &hs_id(), &user)
            .await
            .unwrap();

        let cursors: Vec<u64> = events.iter().map(|event| event.cursor).collect();
        assert_eq!(cursors, [1, 2]);
        assert!(logs_contain("more than the requested limit"));
    }

    /// A failure after the response headers fails the read as a transport error, which
    /// aborts the homeserver's run.
    #[tokio::test]
    async fn user_event_stream_error_is_a_transport_failure() {
        let error = pubky::Error::from(pubky::errors::RequestError::Validation {
            message: "SSE stream error: unexpected EOF".into(),
        });
        let user = pubky::Keypair::random().public_key();
        let mut stream = futures::stream::iter([stream_event(1), Err(error)]);

        let kind = read_user_events(&mut stream, 10, &hs_id(), &user)
            .await
            .unwrap_err();

        assert!(matches!(kind, HsErrorKind::Transport(_)), "{kind:?}");
    }

    // --- user_events_with ---

    /// A 5xx or a transport failure on the `/events-stream` request stops the homeserver's
    /// run, unlike the same failure on a file fetch.
    #[tokio::test]
    async fn user_event_stream_failures_stop_the_run() {
        let user = pubky::Keypair::random().public_key();
        for kind in [
            HsErrorKind::ServerError(530, "".into()),
            HsErrorKind::transport("connection reset"),
        ] {
            let error = user_events_with(&hs_id(), &user, || {
                let kind = kind.clone();
                async move { Err(kind) }
            })
            .await
            .unwrap_err();

            let error = crate::errors::EventProcessorError::from(error);
            assert!(
                matches!(
                    error,
                    crate::errors::EventProcessorError::HsEventsStreamServerError(_)
                        | crate::errors::EventProcessorError::HsEventsStreamTransportFailed(_)
                ),
                "{error:?}"
            );
            assert!(error.should_not_retry_now(), "{error:?}");
        }
    }

    /// A rate-limited `/events-stream` request is retried, and only a 429 that outlasts the
    /// retries stops the homeserver's run.
    #[tokio::test(start_paused = true)]
    async fn user_events_retry_a_rate_limited_stream_request() {
        let user = pubky::Keypair::random().public_key();
        let fetch_failing = |failures: usize, calls: &AtomicUsize| {
            let call = calls.fetch_add(1, Ordering::SeqCst);
            let outcome = if call < failures {
                Err(rate_limited())
            } else {
                Ok(vec![])
            };
            async move { outcome }
        };

        let calls = AtomicUsize::new(0);
        let result = user_events_with(&hs_id(), &user, || fetch_failing(2, &calls)).await;
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(calls.load(Ordering::SeqCst), 3);

        let calls = AtomicUsize::new(0);
        let error = user_events_with(&hs_id(), &user, || fetch_failing(usize::MAX, &calls))
            .await
            .unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        let error = crate::errors::EventProcessorError::from(error);
        assert!(
            matches!(
                error,
                crate::errors::EventProcessorError::HsEventsStreamRateLimitExhausted
            ),
            "{error:?}"
        );
        assert!(error.should_not_retry_now());
    }

    // --- get_resource_with / get_blob_with ---

    /// A 5xx or a transport failure fetching a resource or a blob, which any user can point
    /// at any host, stays one event's failure and does not stop the run.
    #[tokio::test]
    async fn resource_and_blob_failures_stay_per_event() {
        use crate::errors::EventProcessorError;
        use nexus_common::db::PubkyClientError;

        let owner = pubky::Keypair::random().public_key();
        let resource: PubkyResource = format!("pubky://{}/pub/pubky.app/files/0001", owner.z32())
            .parse()
            .expect("valid resource");
        let failures = || {
            [
                HsErrorKind::ServerError(500, "".into()),
                HsErrorKind::transport("connection refused"),
            ]
        };

        // The fetch fails before any body is read, so the blob cap plays no part.
        let unused_cap = 1024;
        let mut errors = Vec::new();
        for kind in failures() {
            errors.push(get_resource_with(&resource, async { Err(kind) }).await);
        }
        for kind in failures() {
            errors.push(get_blob_with(&resource, unused_cap, async { Err(kind) }).await);
        }

        for error in errors {
            let error = EventProcessorError::from(error.unwrap_err());
            assert!(
                matches!(
                    &error,
                    EventProcessorError::PubkyClientError(e) if matches!(
                        e.as_ref(),
                        PubkyClientError::ServerError5xx { .. } | PubkyClientError::RequestFailed { .. }
                    )
                ),
                "{error:?}"
            );
            assert!(!error.should_not_retry_now(), "{error:?}");
        }
    }

    /// A JSON resource is read up to [`MAX_RESOURCE_SIZE`] bytes and no further.
    #[tokio::test]
    async fn resources_are_capped_at_max_resource_size() {
        use crate::errors::EventProcessorError;

        let owner = pubky::Keypair::random().public_key();
        let resource: PubkyResource = format!("pubky://{}/pub/pubky.app/posts/0001", owner.z32())
            .parse()
            .expect("valid resource");

        let at_cap = response(200, vec![0u8; MAX_RESOURCE_SIZE]);
        let body = get_resource_with(&resource, async { Ok(at_cap) })
            .await
            .expect("a body at the cap is accepted");
        assert_eq!(body.len(), MAX_RESOURCE_SIZE);

        let over_cap = response(200, vec![0u8; MAX_RESOURCE_SIZE + 1]);
        let error = get_resource_with(&resource, async { Ok(over_cap) })
            .await
            .unwrap_err();
        let error = EventProcessorError::from(error);
        assert!(
            matches!(error, EventProcessorError::FetchSizeExceeded(_, max) if max == MAX_RESOURCE_SIZE as u64),
            "{error:?}"
        );
    }

    // --- retry_rate_limited ---

    /// Calls `retry_rate_limited` over a fetch that fails with `failures` in order,
    /// then succeeds, and returns its result and the number of calls it made.
    async fn retry_over(failures: Vec<HsErrorKind>) -> (Result<(), HsErrorKind>, usize) {
        let user = pubky::Keypair::random().public_key();
        let calls = AtomicUsize::new(0);

        let result = retry_rate_limited(&user, || {
            let call = calls.fetch_add(1, Ordering::SeqCst);
            let outcome = failures.get(call).cloned().map_or(Ok(()), Err);
            async move { outcome }
        })
        .await;

        (result, calls.load(Ordering::SeqCst))
    }

    fn rate_limited() -> HsErrorKind {
        HsErrorKind::RateLimited("slow down".into())
    }

    /// A 429 is retried after 1 s, then 2 s, and the fetch succeeds on the third call.
    #[tokio::test(start_paused = true)]
    async fn rate_limited_fetch_is_retried_with_backoff() {
        let started = Instant::now();

        let (result, calls) = retry_over(vec![rate_limited(), rate_limited()]).await;

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(calls, 3);
        assert_eq!(started.elapsed(), Duration::from_secs(1 + 2));
    }

    /// Once the three retries are used up, the last 429 is returned.
    #[tokio::test(start_paused = true)]
    async fn rate_limited_fetch_gives_up_after_three_retries() {
        let (result, calls) = retry_over(vec![rate_limited(); 5]).await;

        assert!(
            matches!(result, Err(HsErrorKind::RateLimited(_))),
            "{result:?}"
        );
        assert_eq!(calls, 4, "the first call and three retries");
    }

    /// Any other failure is returned at once.
    #[tokio::test(start_paused = true)]
    async fn other_failures_are_not_retried() {
        let (result, calls) = retry_over(vec![HsErrorKind::ServerError(530, String::new())]).await;

        assert!(
            matches!(result, Err(HsErrorKind::ServerError(530, _))),
            "{result:?}"
        );
        assert_eq!(calls, 1);
    }
}
