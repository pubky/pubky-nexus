use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};

use tracing::Instrument;

// middleware for tracing
pub async fn tracing_middleware(request: Request, next: Next) -> Response {
    let route = request.uri().path().to_string();
    let route_pattern = request.extensions().get::<MatchedPath>();
    let span_name = match route_pattern {
        Some(pattern) => pattern.as_str().to_string(),
        _ => route.clone(),
    };
    let query = request.uri().query().unwrap_or("").to_string();
    let method = request.method().to_string();

    let span = tracing::info_span!(
        "http.request",
        otel.name = %span_name,
        http.request.method = %method,
        http.route = %route,
        http.query = %query,
        http.response.status_code = tracing::field::Empty,
        otel.status_code = tracing::field::Empty,
        otel.status_message = tracing::field::Empty,
    );

    let response = next.run(request).instrument(span.clone()).await;

    let status = response.status().as_u16();
    span.record("http.response.status_code", status);
    if (500..=599).contains(&status) {
        span.record("otel.status_code", "ERROR");
        span.record("otel.status_message", "Internal Server Error");
    }

    response
}
