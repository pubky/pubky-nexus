//! HTTP tracing and metrics middleware.
//!
//! Incoming W3C `traceparent` is intentionally ignored. This is a public
//! unauthenticated API, so clients must not inject trace context; each request
//! starts a new root span via the tracing subscriber.

use std::{sync::LazyLock, time::Instant};

use axum::{
    extract::{MatchedPath, Request},
    http::Method,
    middleware::Next,
    response::Response,
};
use opentelemetry::metrics::{Counter, Histogram};
use opentelemetry::{global, KeyValue};
use tracing::Instrument;

const METER_NAME: &str = "nexus";

/// Request total, availability failures, and time until response headers are ready.
/// Instruments are no-ops until an `SdkMeterProvider` is installed.
struct HttpMetrics {
    requests: Counter<u64>,
    errors: Counter<u64>,
    duration: Histogram<f64>,
}

impl HttpMetrics {
    fn new() -> Self {
        let meter = global::meter(METER_NAME);
        Self {
            requests: meter
                .u64_counter("http.server.requests")
                .with_description(
                    "Total HTTP requests handled by Nexus, by method/route/status",
                )
                .build(),
            errors: meter
                .u64_counter("http.server.errors")
                .with_description(
                    "HTTP requests that failed from the server's perspective (5xx and 408 timeouts), by method/route/status",
                )
                .build(),
            duration: meter
                .f64_histogram("http.server.request.duration")
                .with_description("Duration of HTTP server requests")
                .with_unit("s")
                // Explicit buckets from the HTTP semantic conventions (seconds).
                .with_boundaries(vec![
                    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5,
                    10.0,
                ])
                .build(),
        }
    }
}

/// Lazy on purpose: instruments bind to whatever meter provider is global at
/// creation time, and stay no-ops forever if that is still the default one.
/// First use is the first HTTP request, safely after `setup_metrics`.
static METRICS: LazyLock<HttpMetrics> = LazyLock::new(HttpMetrics::new);

/// Availability failure: 5xx, plus 408 because it times out *our* work.
///
/// Other 4xx (bad input, missing entities, blacklist 403, oversized 413,
/// rate-limit 429) are ordinary on a public unauthenticated API. They stay on
/// `http.server.requests` via `http.response.status_code` without polluting
/// the error rate — same policy as `Error` logging in `error.rs`.
fn is_failed_request(status: u16) -> bool {
    matches!(status, 408 | 500..=599)
}

/// Low-cardinality route template, or `unmatched`. Never the raw URI:
/// `/v0/user/<pubkey>` would explode series cardinality on scanner 404s.
///
/// `MatchedPath` is only set for middleware on the router that declared the
/// routes (see `build_app`). A nested copy of this layer would see `unmatched`.
fn http_route(request: &Request) -> String {
    request
        .extensions()
        .get::<MatchedPath>()
        .map(|pattern| pattern.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string())
}

/// Known methods per OTel semconv, `_OTHER` for the rest. Clients can send
/// arbitrary extension methods, which would otherwise mint unbounded metric
/// series. The raw value goes on the span as `http.request.method_original`.
fn http_method(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::HEAD => "HEAD",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::DELETE => "DELETE",
        Method::CONNECT => "CONNECT",
        Method::OPTIONS => "OPTIONS",
        Method::TRACE => "TRACE",
        Method::PATCH => "PATCH",
        _ => "_OTHER",
    }
}

fn record_http_request(method: &str, route: &str, status: u16, elapsed_secs: f64) {
    let attrs = [
        KeyValue::new("http.request.method", method.to_string()),
        KeyValue::new("http.route", route.to_string()),
        KeyValue::new("http.response.status_code", i64::from(status)),
    ];
    METRICS.requests.add(1, &attrs);
    METRICS.duration.record(elapsed_secs, &attrs);
    if is_failed_request(status) {
        METRICS.errors.add(1, &attrs);
    }
}

/// Root span plus request/error/duration metrics for every request.
pub async fn tracing_middleware(request: Request, next: Next) -> Response {
    let route = http_route(&request);
    let method = http_method(request.method());

    // Query strings are deliberately not recorded: they carry search text,
    // pubkeys, and filter values, with no cardinality bound in Tempo.
    let span = tracing::info_span!(
        "http.request",
        // Semconv wants a bare `{method}` when no route matched; we keep
        // `unmatched` so scanner 404s stay searchable in Tempo.
        otel.name = %format!("{method} {route}"),
        http.request.method = %method,
        http.request.method_original = tracing::field::Empty,
        http.route = %route,
        http.response.status_code = tracing::field::Empty,
        otel.status_code = tracing::field::Empty,
        otel.status_message = tracing::field::Empty,
    );
    if method == "_OTHER" {
        span.record("http.request.method_original", request.method().to_string());
    }

    let started = Instant::now();
    let response = next.run(request).instrument(span.clone()).await;

    let status = response.status();
    span.record("http.response.status_code", status.as_u16());
    if is_failed_request(status.as_u16()) {
        span.record("otel.status_code", "ERROR");
        span.record(
            "otel.status_message",
            status.canonical_reason().unwrap_or("error"),
        );
    }

    record_http_request(
        method,
        &route,
        status.as_u16(),
        started.elapsed().as_secs_f64(),
    );

    response
}

#[cfg(test)]
mod tests {
    use super::{http_method, http_route, is_failed_request, Method, Request};
    use axum::body::Body;

    // The matched case can't be unit-tested: `MatchedPath` has no public
    // constructor and only exists inside a real router.
    #[test]
    fn route_without_matched_path_is_unmatched() {
        let request = Request::builder()
            .uri("/wp-admin")
            .body(Body::empty())
            .unwrap();
        assert_eq!(http_route(&request), "unmatched");
    }

    #[test]
    fn extension_methods_collapse_to_other() {
        assert_eq!(http_method(&Method::GET), "GET");
        assert_eq!(http_method(&Method::PATCH), "PATCH");

        let custom = Method::from_bytes(b"PROPFIND").unwrap();
        assert_eq!(http_method(&custom), "_OTHER");
    }

    #[test]
    fn failed_request_is_5xx_and_timeout_only() {
        assert!(is_failed_request(500));
        assert!(is_failed_request(502));
        assert!(is_failed_request(503));
        assert!(is_failed_request(408));

        assert!(!is_failed_request(200));
        assert!(!is_failed_request(204));
        assert!(!is_failed_request(400));
        assert!(!is_failed_request(403));
        assert!(!is_failed_request(404));
        assert!(!is_failed_request(413));
        assert!(!is_failed_request(429));
    }
}
