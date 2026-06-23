use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::http::{header, Response, StatusCode};
use nexus_common::RateLimitConfig;
use tokio::sync::watch::Receiver;
use tower_governor::errors::GovernorError;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::{KeyExtractor, PeerIpKeyExtractor, SmartIpKeyExtractor};
use tower_governor::GovernorLayer;
use tracing::{debug, error};

/// A unified IP key extractor that delegates to either peer-IP or forwarded-header extraction
/// depending on how it was constructed.
///
/// Using a single concrete extractor type avoids the generic divergence that would otherwise
/// arise from branching on `SmartIpKeyExtractor` vs `PeerIpKeyExtractor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IpKeyExtractor {
    /// Use TCP peer address only (safe against header spoofing).
    Peer,
    /// Trust forwarded-IP headers (requires a known reverse-proxy in front).
    Smart,
}

impl KeyExtractor for IpKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        match self {
            IpKeyExtractor::Peer => PeerIpKeyExtractor.extract(req),
            IpKeyExtractor::Smart => SmartIpKeyExtractor.extract(req),
        }
    }
}

/// Compute the token-bucket refill period from a per-minute rate.
///
/// Uses millisecond arithmetic to avoid the integer-division truncation that
/// occurs with `Duration::from_secs(60 / rate)` for rates 31–59.
fn compute_refill_period(rate_per_min: u32) -> Duration {
    Duration::from_millis((60_000 / rate_per_min as u64).max(1))
}

/// Apply a `GovernorLayer` for the given bucket config.
/// Returns `router` unchanged when rate limiting is disabled or burst/rate are zero.
///
/// **Security:** When `trust_proxy_headers` is `true`, forwarded-IP headers
/// are consulted (behind a known reverse proxy). When `false`, only the real
/// TCP peer address is used — preventing trivial client-side XFF spoofing.
fn apply_rate_limit_bucket<S>(
    router: axum::Router<S>,
    rate_per_min: u32,
    burst: u32,
    bucket_label: &'static str,
    trust_proxy_headers: bool,
    shutdown_rx: Receiver<bool>,
) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if rate_per_min == 0 || burst == 0 {
        return router;
    }

    let key_extractor = if trust_proxy_headers {
        IpKeyExtractor::Smart
    } else {
        IpKeyExtractor::Peer
    };

    let refill_period = compute_refill_period(rate_per_min);

    let Some(config) = GovernorConfigBuilder::default()
        .period(refill_period)
        .burst_size(burst)
        .key_extractor(key_extractor)
        .finish()
    else {
        return router;
    };

    let limiter = config.limiter().clone();
    tokio::spawn(async move {
        let mut shutdown_rx = shutdown_rx;
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = interval.tick() => { limiter.retain_recent(); }
                _ = shutdown_rx.changed() => {
                    debug!("Rate-limit cleanup task shutting down (bucket={})", bucket_label);
                    break;
                }
            }
        }
    });

    let counter = Arc::new(
        opentelemetry::global::meter("nexus")
            .u64_counter("http.rate_limit.rejected.total")
            .with_description("Total number of rate-limited (429) responses")
            .build(),
    );

    router.layer(
        GovernorLayer::new(config).error_handler(move |err: GovernorError| match &err {
            GovernorError::TooManyRequests { wait_time, .. } => {
                let wait_secs = *wait_time;
                counter.add(1, &[opentelemetry::KeyValue::new("bucket", bucket_label)]);
                debug!("Rate limit exceeded (bucket={bucket_label}) — retry after {wait_secs}s");
                let mut response = Response::new(axum::body::Body::empty());
                *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
                response
                    .headers_mut()
                    .insert(header::RETRY_AFTER, wait_secs.to_string().parse().unwrap());
                response
            }
            _ => {
                error!("Rate-limit key extraction failed (bucket={bucket_label}): {err}");
                let mut response = Response::new(axum::body::Body::empty());
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                response
            }
        }),
    )
}

/// Apply the expensive-bucket rate-limit layer to `router`.
/// Returns `router` unchanged when rate limiting is disabled.
pub fn apply_rate_limit_expensive<S>(
    router: axum::Router<S>,
    config: &RateLimitConfig,
    shutdown_rx: Receiver<bool>,
) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if !config.enabled {
        return router;
    }
    apply_rate_limit_bucket(
        router,
        config.expensive_bucket.rate,
        config.expensive_bucket.burst,
        "expensive",
        config.trust_proxy_headers,
        shutdown_rx,
    )
}

/// Apply the default-bucket rate-limit layer to `router`.
/// Returns `router` unchanged when rate limiting is disabled.
pub fn apply_rate_limit_default<S>(
    router: axum::Router<S>,
    config: &RateLimitConfig,
    shutdown_rx: Receiver<bool>,
) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if !config.enabled {
        return router;
    }
    apply_rate_limit_bucket(
        router,
        config.default_bucket.rate,
        config.default_bucket.burst,
        "default",
        config.trust_proxy_headers,
        shutdown_rx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn zero_rate_or_burst_is_noop() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use axum::routing::get;
        use tower::ServiceExt;

        // No ConnectInfo on the request: if a GovernorLayer were applied it would
        // fail key extraction and return 500, so all-200 proves no layer was added.
        for (rate, burst) in [(0u32, 50u32), (300, 0)] {
            let (_, rx) = tokio::sync::watch::channel(false);
            let router = axum::Router::new().route("/", get(|| async { "ok" }));
            let router = apply_rate_limit_bucket(router, rate, burst, "test", false, rx);

            for _ in 0..10 {
                let resp = router
                    .clone()
                    .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
            }
        }
    }

    #[tokio::test]
    async fn enabled_builds_layer() {
        use axum::body::Body;
        use axum::extract::ConnectInfo;
        use axum::http::{Request, StatusCode};
        use axum::routing::get;
        use std::net::SocketAddr;
        use tower::ServiceExt;

        let (_, shutdown_rx) = tokio::sync::watch::channel(false);
        let router = axum::Router::new().route("/", get(|| async { "ok" }));
        // burst=1: first request consumes the single token, second is rejected.
        let router = apply_rate_limit_bucket(router, 60, 1, "test", false, shutdown_rx);

        let peer: SocketAddr = "127.0.0.1:1234".parse().unwrap();

        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(ConnectInfo::<SocketAddr>(peer));
        assert_eq!(
            router.clone().oneshot(req).await.unwrap().status(),
            StatusCode::OK
        );

        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(ConnectInfo::<SocketAddr>(peer));
        assert_eq!(
            router.clone().oneshot(req).await.unwrap().status(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn refill_period_is_accurate_for_all_rates() {
        // rate 40 → 60_000 / 40 = 1500 ms (old bug: 60/40=1s → enforced 60 req/min)
        assert_eq!(compute_refill_period(40), Duration::from_millis(1500));

        // rate 31 → 60_000 / 31 = 1935 ms (old bug: truncated to 1s)
        assert_eq!(compute_refill_period(31), Duration::from_millis(1935));

        // rate 59 → 60_000 / 59 = 1016 ms (old bug: truncated to 1s)
        assert_eq!(compute_refill_period(59), Duration::from_millis(1016));

        // rate 60 → exactly 1000 ms (boundary, was correct in the old >=60 branch)
        assert_eq!(compute_refill_period(60), Duration::from_millis(1000));

        // rate 120 → 500 ms (sub-second, requires millis arithmetic)
        assert_eq!(compute_refill_period(120), Duration::from_millis(500));

        // rate 1 → 60_000 ms = 60s (lowest valid rate)
        assert_eq!(compute_refill_period(1), Duration::from_millis(60_000));
    }
}
