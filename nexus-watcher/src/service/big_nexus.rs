//! Reading another Nexus's public API, as the seed bootstrap and the inbound bridge do.
//!
//! One request per call, so the caller can count each one and decide how to retry; each
//! bounded in time (headers and body alike) and in size, whatever the other Nexus does.

use std::future::Future;
use std::time::Duration;

use nexus_common::db::PubkyConnector;
use nexus_common::types::DynError;
use pubky::Method;
use serde::de::DeserializeOwned;

use crate::homeserver::fetch::{fetch_capped, MAX_RESOURCE_SIZE};

/// How long one request may take, body included.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest a rate limit (`429`) is waited out, whatever `Retry-After` says.
pub(crate) const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);
/// How long a rate-limited request waits when the Nexus doesn't say.
const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(5);

/// What one request to another Nexus answered.
#[derive(Debug, Clone, PartialEq)]
pub enum NexusAnswer<T> {
    Found(T),
    /// A `404`: the Nexus doesn't know what was asked for.
    NotFound,
    /// A `429`: ask again after this long (at most [`MAX_RATE_LIMIT_WAIT`]).
    RateLimited(Duration),
}

/// GETs `url` from another Nexus and reads its JSON body.
pub(crate) async fn get_json<T: DeserializeOwned>(url: &str) -> Result<NexusAnswer<T>, DynError> {
    let pubky = PubkyConnector::get()?;
    let request = pubky.client().request(Method::GET, &url.to_owned()).send();
    read_answer(url, request).await
}

/// One request, from `send` to the end of its body, within [`REQUEST_TIMEOUT`]: a Nexus
/// that sends headers and stalls its body is cut off too.
pub(crate) async fn read_answer<T: DeserializeOwned>(
    url: &str,
    send: impl Future<Output = Result<reqwest::Response, reqwest::Error>>,
) -> Result<NexusAnswer<T>, DynError> {
    let read = async {
        let response = send.await?;
        match response.status() {
            reqwest::StatusCode::NOT_FOUND => Ok(NexusAnswer::NotFound),
            reqwest::StatusCode::TOO_MANY_REQUESTS => {
                Ok(NexusAnswer::RateLimited(retry_after(&response)))
            }
            status if !status.is_success() => Err(format!("GET {url}: HTTP {status}").into()),
            _ => {
                let body = fetch_capped(response, MAX_RESOURCE_SIZE as u64)
                    .await
                    .map_err(|e| format!("GET {url}: {e:?}"))?;
                Ok(NexusAnswer::Found(serde_json::from_slice(&body)?))
            }
        }
    };
    tokio::time::timeout(REQUEST_TIMEOUT, read)
        .await
        .map_err(|_| format!("GET {url}: timed out"))?
}

/// How long a `429` asks to wait (`Retry-After` in seconds), at most
/// [`MAX_RATE_LIMIT_WAIT`].
fn retry_after(response: &reqwest::Response) -> Duration {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok()?.parse::<u64>().ok())
        .map_or(DEFAULT_RATE_LIMIT_WAIT, Duration::from_secs)
        .min(MAX_RATE_LIMIT_WAIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body that never arrives is cut off by the same timeout as the headers.
    #[tokio::test(start_paused = true)]
    async fn a_stalled_body_times_out() {
        let stalled = futures::stream::pending::<Result<bytes::Bytes, std::io::Error>>();
        let response =
            reqwest::Response::from(http::Response::new(reqwest::Body::wrap_stream(stalled)));

        // A deadline of its own, so the test fails rather than hangs if the body is read
        // outside the request's timeout.
        let deadline = REQUEST_TIMEOUT + Duration::from_secs(1);
        let read = tokio::time::timeout(
            deadline,
            read_answer::<Vec<String>>("url", std::future::ready(Ok(response))),
        )
        .await
        .expect("cut off within the request's timeout");

        assert!(read.unwrap_err().to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn reads_each_kind_of_answer() {
        let answer = |status: u16, retry_after: Option<&str>, body: &'static str| {
            let mut builder = http::Response::builder().status(status);
            if let Some(seconds) = retry_after {
                builder = builder.header("retry-after", seconds);
            }
            let response = reqwest::Response::from(builder.body(body).unwrap());
            read_answer::<Vec<String>>("url", std::future::ready(Ok(response)))
        };

        assert_eq!(
            answer(200, None, "[\"a\"]").await.unwrap(),
            NexusAnswer::Found(vec!["a".to_string()])
        );
        assert_eq!(answer(404, None, "").await.unwrap(), NexusAnswer::NotFound);
        assert_eq!(
            answer(429, Some("7"), "").await.unwrap(),
            NexusAnswer::RateLimited(Duration::from_secs(7))
        );
        assert_eq!(
            answer(429, Some("100000"), "").await.unwrap(),
            NexusAnswer::RateLimited(MAX_RATE_LIMIT_WAIT),
            "at most the longest wait"
        );
        assert!(answer(500, None, "").await.is_err());
    }
}
