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

/// Bound after `setup_metrics`; no-ops if OTLP was never configured.
static METRICS: LazyLock<HttpMetrics> = LazyLock::new(HttpMetrics::new);

/// 5xx, plus 408 (our timeout). Other 4xx stay on `requests` by status code.
fn is_failed_request(status: u16) -> bool {
    matches!(status, 408 | 500..=599)
}

/// Matched route template, or `unmatched`. Never the raw URI (cardinality).
/// Only set when this middleware sits on the router that declared the routes.
fn http_route(request: &Request) -> String {
    request
        .extensions()
        .get::<MatchedPath>()
        .map(|pattern| pattern.as_str().to_string())
        .unwrap_or_else(|| "unmatched".to_string())
}

/// Semconv well-known methods; anything else is `_OTHER` (raw value on the span).
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

pub async fn tracing_middleware(request: Request, next: Next) -> Response {
    let route = http_route(&request);
    let method = http_method(request.method());

    // No query string: search text / pubkeys / filters, unbounded in Tempo.
    let span = tracing::info_span!(
        "http.request",
        // Semconv would use a bare `{method}` when unmatched; keep the token
        // so scanner 404s stay searchable.
        otel.name = %format!("{method} {route}"),
        http.request.method = %method,
        http.request.method_original = tracing::field::Empty,
        http.route = %route,
        http.response.status_code = tracing::field::Empty,
        otel.status_code = tracing::field::Empty,
        otel.status_message = tracing::field::Empty,
    );
    if method == "_OTHER" {
        span.record("http.request.method_original", request.method().as_str());
    }

    let started = Instant::now();
    let response = next.run(request).instrument(span.clone()).await;

    let status = response.status();
    let status_code = status.as_u16();
    span.record("http.response.status_code", status_code);
    if is_failed_request(status_code) {
        span.record("otel.status_code", "ERROR");
        span.record(
            "otel.status_message",
            status.canonical_reason().unwrap_or("error"),
        );
    }

    record_http_request(method, &route, status_code, started.elapsed().as_secs_f64());

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
