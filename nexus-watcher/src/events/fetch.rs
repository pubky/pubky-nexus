use futures::StreamExt;
use nexus_common::models::event::EventProcessorError;

/// Max bytes to read from an error response body.
pub(crate) const MAX_ERROR_BODY: usize = 4 * 1024;

/// Max bytes for a JSON resource descriptor (user, post, tag, file meta, etc).
pub(crate) const MAX_RESOURCE_SIZE: usize = 2 * 1024 * 1024;

/// Max bytes for a homeserver `/events` response body.
/// Worst case: 1 000 events × 4 160 bytes/line ≈ 4 MiB; 5 MiB gives headroom.
pub(crate) const MAX_EVENTS_BODY: usize = 5 * 1024 * 1024;

/// Truncates a byte slice to `max` bytes for safe embedding in error messages.
pub(crate) fn format_error_body(bytes: &[u8], max: usize) -> String {
    if bytes.len() > max {
        format!("{}… (truncated)", String::from_utf8_lossy(&bytes[..max]))
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Reads chunks from a byte stream, buffering at most `max + 1` bytes.
/// Returns `Ok((bytes, exceeded))` on completion, `Err(e)` on stream failure.
pub(crate) async fn read_stream_capped<S, E>(
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
        buf.extend_from_slice(&bytes[..bytes.len().min(cap.saturating_sub(buf.len()))]);
        if total >= cap {
            return Ok((buf, true));
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
pub(crate) async fn fetch_capped(
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use nexus_common::models::event::EventProcessorError;

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
    async fn read_stream_capped_single_large_chunk() {
        // A chunk much larger than max must not bloat buf beyond cap (max + 1).
        let (buf, exceeded) = read_stream_capped(ok_stream(vec![1; 1_000_000]), 100)
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
