use futures::StreamExt;
use nexus_common::db::PubkyConnector;
use nexus_common::models::event::{Event, EventProcessorError, EventType};
use pubky_app_specs::{PubkyAppObject, Resource};
use std::sync::Arc;
use tracing::debug;

pub mod handlers;
mod moderation;
pub mod retry;

pub use moderation::Moderation;

/// Max bytes to read from an error response body.
pub(super) const MAX_ERROR_BODY: usize = 4 * 1024;

/// Max bytes for a JSON resource descriptor (user, post, tag, file meta, etc).
pub(super) const MAX_RESOURCE_SIZE: usize = 2 * 1024 * 1024;

/// Truncates a byte slice to `max` bytes for safe embedding in error messages.
pub(super) fn format_error_body(bytes: &[u8], max: usize) -> String {
    if bytes.len() > max {
        format!("{}… (truncated)", String::from_utf8_lossy(&bytes[..max]))
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Reads chunks from a byte stream, stopping after `max + 1` bytes.
/// Returns `Ok((bytes, exceeded))` on completion, `Err(e)` on stream failure.
pub(super) async fn read_stream_capped<S, E>(
    mut stream: S,
    max: usize,
) -> Result<(Vec<u8>, bool), E>
where
    S: futures::Stream<Item = Result<bytes::Bytes, E>> + Unpin,
{
    let cap = max + 1;
    let mut buf = Vec::new();
    let mut total = 0usize;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        total += bytes.len();
        buf.extend_from_slice(&bytes);
        if total >= cap {
            return Ok((buf, total > max));
        }
    }

    Ok((buf, false))
}

/// Fetches the body of a `reqwest::Response`, enforcing a size limit.
///
/// 1. If `Content-Length` is present and exceeds `max`, rejects immediately.
/// 2. Otherwise, streams through [read_stream_capped] to catch lying/missing
///    `Content-Length` headers.
///
/// Returns [EventProcessorError::FetchSizeExceeded] on size violation,
/// [EventProcessorError::ClientError] on stream failure.
pub(super) async fn fetch_capped(
    resp: reqwest::Response,
    max: u64,
) -> Result<Vec<u8>, EventProcessorError> {
    if let Some(cl) = resp.content_length() {
        if cl > max {
            return Err(EventProcessorError::FetchSizeExceeded(cl, max));
        }
    }
    let (buf, exceeded) = read_stream_capped(resp.bytes_stream(), max as usize)
        .await
        .map_err(|e| EventProcessorError::client_error(e.to_string()))?;
    if exceeded {
        return Err(EventProcessorError::FetchSizeExceeded(
            buf.len() as u64,
            max,
        ));
    }
    Ok(buf)
}

pub async fn handle(
    event: &Event,
    moderation: Arc<Moderation>,
    max_file_size: u64,
) -> Result<(), EventProcessorError> {
    match event.event_type {
        EventType::Put => handle_put_event(event, moderation, max_file_size).await,
        EventType::Del => handle_del_event(event).await,
    }?;

    event.store_event().await?;
    Ok(())
}

pub async fn handle_put_event(
    event: &Event,
    moderation: Arc<Moderation>,
    max_file_size: u64,
) -> Result<(), EventProcessorError> {
    debug!("Handling PUT event for URI: {}", event.uri);

    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(&event.uri).await?;

    if !response.status().is_success() {
        let status = response.status();
        let (body, _exceeded) = read_stream_capped(response.bytes_stream(), MAX_ERROR_BODY)
            .await
            .unwrap_or_default();
        let body = format_error_body(&body, MAX_ERROR_BODY);

        let err_msg = format!(
            "Fetch resource failed {}: HTTP {status} - {body}",
            event.uri
        );
        return Err(EventProcessorError::client_error(err_msg))?;
    }

    let blob = fetch_capped(response, MAX_RESOURCE_SIZE as u64).await?;

    let resource = event.parsed_uri.resource.clone();

    // Use the new importer from pubky-app-specs.
    // `from_resource` runs spec validation; failures are deterministic and must
    // not be retried (a re-run produces the same error). Classify them as
    // `SpecValidation` so the retry queue stays clean — the load-bearing
    // counterpart to the `Unknown` forwards-compat variant in pubky-app-specs.
    let pubky_object = PubkyAppObject::from_resource(&resource, blob.as_slice())
        .map_err(|e| EventProcessorError::SpecValidation(e.to_string()))?;

    let user_id = event.parsed_uri.user_id.clone();
    match (pubky_object, resource) {
        (PubkyAppObject::User(user), Resource::User) => {
            handlers::user::sync_put(user, user_id).await?
        }
        (PubkyAppObject::Post(post), Resource::Post(post_id)) => {
            handlers::post::sync_put(post, user_id, post_id).await?
        }
        (PubkyAppObject::Follow(_follow), Resource::Follow(followee_id)) => {
            handlers::follow::sync_put(user_id, followee_id).await?
        }
        (PubkyAppObject::Mute(_), Resource::Mute(_)) => {
            debug!("Mute events are no longer handled by nexus");
        }
        (PubkyAppObject::Bookmark(bookmark), Resource::Bookmark(bookmark_id)) => {
            handlers::bookmark::sync_put(user_id, bookmark, bookmark_id).await?
        }
        (PubkyAppObject::Tag(tag), Resource::Tag(tag_id)) => {
            if moderation.should_delete(&tag, user_id.clone()).await {
                Moderation::apply_moderation(tag, event.files_path.clone()).await?
            } else {
                handlers::tag::sync_put(tag, user_id, tag_id).await?
            }
        }
        (PubkyAppObject::File(file), Resource::File(file_id)) => {
            handlers::file::sync_put(
                file,
                event.uri.clone(),
                user_id,
                file_id,
                event.files_path.clone(),
                max_file_size,
            )
            .await?
        }
        other => debug!("Event type not handled, Resource: {other:?}"),
    }
    Ok(())
}

/// Handles a DEL event by dispatching to the appropriate handler.
pub async fn handle_del_event(event: &Event) -> Result<(), EventProcessorError> {
    debug!("Handling DEL event for URI: {}", event.uri);

    let user_id = event.parsed_uri.user_id.clone();
    match &event.parsed_uri.resource {
        Resource::User => handlers::user::del(user_id).await?,
        Resource::Post(post_id) => handlers::post::del(user_id, post_id.clone()).await?,
        Resource::Follow(followee_id) => {
            handlers::follow::del(user_id, followee_id.clone()).await?
        }
        Resource::Mute(_) => debug!("Mute events are no longer handled by nexus"),
        Resource::Bookmark(bookmark_id) => {
            handlers::bookmark::del(user_id, bookmark_id.clone()).await?
        }
        Resource::Tag(_) => handlers::tag::del(&event.uri).await?,
        Resource::File(file_id) => {
            handlers::file::del(&user_id, file_id.clone(), event.files_path.clone()).await?
        }
        other => debug!("DEL event type not handled for resource: {other:?}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    /// Trivial error type so we can construct a failing stream without reqwest internals.
    #[derive(Debug)]
    struct TestErr;

    fn ok_stream(
        data: Vec<u8>,
    ) -> impl futures::Stream<Item = Result<bytes::Bytes, TestErr>> + Unpin {
        stream::iter(vec![Ok(bytes::Bytes::from(data))])
    }

    fn err_stream() -> impl futures::Stream<Item = Result<bytes::Bytes, TestErr>> + Unpin {
        stream::iter(vec![Err(TestErr)])
    }

    fn partial_then_err(
        data: Vec<u8>,
    ) -> impl futures::Stream<Item = Result<bytes::Bytes, TestErr>> + Unpin {
        stream::iter(vec![Ok(bytes::Bytes::from(data)), Err(TestErr)])
    }

    #[tokio::test]
    async fn read_stream_capped_empty() {
        let (buf, exceeded) = read_stream_capped(ok_stream(vec![]), 100).await.unwrap();
        assert!(buf.is_empty());
        assert!(!exceeded);
    }

    #[tokio::test]
    async fn read_stream_capped_exact() {
        let (buf, exceeded) = read_stream_capped(ok_stream(vec![1; 100]), 100)
            .await
            .unwrap();
        assert_eq!(buf.len(), 100);
        assert!(!exceeded);
    }

    #[tokio::test]
    async fn read_stream_capped_over() {
        let (buf, exceeded) = read_stream_capped(ok_stream(vec![1; 101]), 100)
            .await
            .unwrap();
        assert_eq!(buf.len(), 101);
        assert!(exceeded);
    }

    #[tokio::test]
    async fn read_stream_capped_propagates_error() {
        let result: Result<(Vec<u8>, bool), TestErr> = read_stream_capped(err_stream(), 100).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn read_stream_capped_propagates_error_after_data() {
        let result: Result<(Vec<u8>, bool), TestErr> =
            read_stream_capped(partial_then_err(vec![1; 50]), 100).await;
        assert!(result.is_err());
    }

    // --- fetch_capped tests ---

    fn no_cl_oversized(total: usize) -> reqwest::Response {
        let body = reqwest::Body::wrap_stream(stream::iter(vec![Ok::<_, std::io::Error>(
            bytes::Bytes::from(vec![0xAB; total]),
        )]));
        reqwest::Response::from(http::Response::new(body))
    }

    fn high_cl_response(total: usize) -> reqwest::Response {
        let body = reqwest::Body::from(vec![0u8; total]);
        reqwest::Response::from(http::Response::new(body))
    }

    #[tokio::test]
    async fn fetch_capped_precheck_rejects_high_cl() {
        let resp = high_cl_response(10_000);
        // Guard: the precheck relies on `Content-Length` being reflected here.
        assert_eq!(resp.content_length(), Some(10_000));

        let err = fetch_capped(resp, 100).await.unwrap_err();
        assert!(matches!(
            err,
            EventProcessorError::FetchSizeExceeded(10_000, 100)
        ));
    }

    #[tokio::test]
    async fn fetch_capped_stream_rejects_absent_cl_oversized() {
        let r = no_cl_oversized(200);
        assert!(
            r.content_length().is_none(),
            "guard: else this re-tests the pre-check"
        );
        let err = fetch_capped(r, 100).await.unwrap_err();
        assert!(matches!(
            err,
            EventProcessorError::FetchSizeExceeded(_, 100)
        ));
    }

    #[tokio::test]
    async fn fetch_capped_accepts_under_cap_stream() {
        assert!(fetch_capped(no_cl_oversized(50), 100).await.is_ok());
    }
}
