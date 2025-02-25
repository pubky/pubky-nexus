use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};

use opentelemetry::{
    global::{self},
    trace::{FutureExt, Span, Status, TraceContextExt, Tracer},
    Context, KeyValue,
};

// middleware for tracing
pub async fn tracing_middleware(request: Request, next: Next) -> Response {
    let tracer = global::tracer("nexus.service");
    let route = request.uri().path().to_string();
    let route_pattern = request.extensions().get::<MatchedPath>();
    let span_name = match route_pattern {
        Some(pattern) => pattern.as_str().to_string(),
        _ => route.clone(),
    };
    let mut span = tracer.start(span_name);
    span.set_attribute(KeyValue::new("http.method", request.method().to_string()));
    span.set_attribute(KeyValue::new("http.route", route));
    span.set_attribute(KeyValue::new(
        "http.query",
        request.uri().query().unwrap_or("").to_string(),
    ));

    let cx = Context::new().with_span(span);

    let response = next.run(request).with_context(cx.clone()).await;

    let span = cx.span();
    let status = response.status().as_u16() as i64;
    span.set_attribute(KeyValue::new("http.status_code", status));
    match status {
        400..=499 => span.set_status(Status::Error {
            description: "Expected Error".into(),
        }),
        500..=599 => span.set_status(Status::Error {
            description: "Internal Server Error".into(),
        }),
        _ => span.set_status(Status::Ok),
    };
    span.end();

    response
}
