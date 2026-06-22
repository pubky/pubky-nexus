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
use tracing::debug;

type RateLimitLayer = GovernorLayer<
    IpKeyExtractor,
    governor::middleware::NoOpMiddleware<governor::clock::QuantaInstant>,
    axum::body::Body,
>;

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

/// Build a `GovernorLayer` for the given bucket config.
/// Returns `None` when rate limiting is disabled or burst/rate are zero.
///
/// **Security:** When `trust_proxy_headers` is `true`, forwarded-IP headers
/// are consulted (behind a known reverse proxy). When `false`, only the real
/// TCP peer address is used — preventing trivial client-side XFF spoofing.
fn build_layer(
    rate_per_min: u32,
    burst: u32,
    bucket_label: &'static str,
    trust_proxy_headers: bool,
    shutdown_rx: Receiver<bool>,
) -> Option<RateLimitLayer> {
    if rate_per_min == 0 || burst == 0 {
        return None;
    }

    let key_extractor = if trust_proxy_headers {
        IpKeyExtractor::Smart
    } else {
        IpKeyExtractor::Peer
    };

    let refill_period = if rate_per_min >= 60 {
        Duration::from_millis((1_000 * 60 / rate_per_min as u64).max(1))
    } else {
        Duration::from_secs((60 / rate_per_min as u64).max(1))
    };

    let config = GovernorConfigBuilder::default()
        .period(refill_period)
        .burst_size(burst)
        .key_extractor(key_extractor)
        .finish()?;

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

    Some(
        GovernorLayer::new(config).error_handler(move |err: GovernorError| {
            counter.add(1, &[opentelemetry::KeyValue::new("bucket", bucket_label)]);
            let wait_secs = match &err {
                GovernorError::TooManyRequests { wait_time, .. } => *wait_time,
                _ => 0,
            };
            debug!("Rate limit exceeded (bucket={bucket_label}) — retry after {wait_secs}s");

            let mut response = Response::new(axum::body::Body::empty());
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, wait_secs.to_string().parse().unwrap());
            response
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
    match build_layer(
        config.expensive_bucket.rate,
        config.expensive_bucket.burst,
        "expensive",
        config.trust_proxy_headers,
        shutdown_rx,
    ) {
        Some(layer) => router.layer(layer),
        None => router,
    }
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
    match build_layer(
        config.default_bucket.rate,
        config.default_bucket.burst,
        "default",
        config.trust_proxy_headers,
        shutdown_rx,
    ) {
        Some(layer) => router.layer(layer),
        None => router,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_rate_or_burst_is_noop() {
        let (_, rx) = tokio::sync::watch::channel(false);
        assert!(build_layer(0, 50, "test", false, rx).is_none());
        let (_, rx) = tokio::sync::watch::channel(false);
        assert!(build_layer(300, 0, "test", false, rx).is_none());
    }

    #[tokio::test]
    async fn enabled_builds_layer() {
        let config = RateLimitConfig {
            enabled: true,
            ..Default::default()
        };
        let (_, shutdown_rx) = tokio::sync::watch::channel(false);
        assert!(build_layer(
            config.default_bucket.rate,
            config.default_bucket.burst,
            "test",
            false,
            shutdown_rx,
        )
        .is_some());
    }
}
