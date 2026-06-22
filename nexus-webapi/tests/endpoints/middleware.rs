use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::body::Body;
use axum::extract::Path;
use axum::http::header::{ACCESS_CONTROL_ALLOW_ORIGIN, CONTENT_LENGTH, ORIGIN};
use axum::http::{Method, Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use nexus_common::RateLimitConfig;
use nexus_webapi::routes::{app_routes, build_app, AppState};
use tempfile::TempDir;
use tokio::sync::watch;
use tower::ServiceExt;
// =============================================
// Request body size limit (RequestBodyLimitLayer)
// =============================================

#[tokio::test]
async fn test_request_body_size_limit() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let state = AppState {
        files_path: Arc::new(temp_dir.path().to_path_buf()),
    };
    let rate_limit_config: RateLimitConfig = RateLimitConfig::default();
    let (_tx, rx) = watch::channel(false);
    let routes = app_routes(state.clone(), &rate_limit_config, rx);

    // 10-byte limit; body well over it.
    let app = build_app(routes, state, 30, 10);

    let body = vec![0u8; 1000];

    let req = Request::builder()
        .method(Method::POST)
        .uri("/v0/files/by_ids")
        .header(CONTENT_LENGTH, body.len().to_string())
        .body(Body::from(body))?;

    let response = app.oneshot(req).await?;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    Ok(())
}

async fn bytes_handler(_body: axum::body::Bytes) -> StatusCode {
    StatusCode::OK
}

/// Without a `Content-Length` header, [RequestBodyLimitLayer] can't reject the
/// request upfront based on the header alone. It instead wraps the body in a
/// limited body that errors once the handler reads past the limit while
/// consuming it, which axum turns into a 413 response.
#[tokio::test]
async fn test_request_body_size_limit_without_content_length() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let state = AppState {
        files_path: Arc::new(temp_dir.path().to_path_buf()),
    };
    let routes = Router::new().route("/echo", post(bytes_handler));

    // 10-byte limit; body well over it, sent without a Content-Length header.
    let app = build_app(routes, state, 30, 10);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/echo")
        .body(Body::from(vec![0u8; 1000]))?;

    assert!(req.headers().get(CONTENT_LENGTH).is_none());

    let response = app.oneshot(req).await?;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    Ok(())
}

// =============================================
// Request timeout (TimeoutLayer)
// =============================================

async fn sleep_handler(Path(millis): Path<u64>) -> StatusCode {
    tokio::time::sleep(Duration::from_millis(millis)).await;
    StatusCode::OK
}

/// A request whose handler exceeds the configured timeout receives 408.
/// Time is paused so the test doesn't wait a real second.
#[tokio::test]
async fn test_request_timeout_returns_408() -> Result<()> {
    tokio::time::pause();

    let temp_dir = TempDir::new()?;
    let state = AppState {
        files_path: Arc::new(temp_dir.path().to_path_buf()),
    };
    let routes = Router::new().route("/sleep/{millis}", get(sleep_handler));

    // 1-second timeout; handler sleeps far longer (time is paused, so no real wait).
    let app = build_app(routes, state, 1, 1024 * 1024);

    let req = Request::builder()
        .uri("/sleep/9999999")
        .body(Body::empty())?;
    let task = tokio::spawn(app.oneshot(req));
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(2)).await;

    let response = task.await??;

    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);

    Ok(())
}

// =============================================
// CORS headers on responses short-circuited by
// RequestBodyLimitLayer / TimeoutLayer
// =============================================

async fn ok_handler() -> StatusCode {
    StatusCode::OK
}

/// If `RequestBodyLimitLayer` were layered outside `CorsLayer`, its 413 response
/// would bypass CORS entirely and a cross-origin browser client couldn't read
/// the status/body. This test guards the layer ordering that prevents that.
#[tokio::test]
async fn test_body_size_limit_response_has_cors_header() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let state = AppState {
        files_path: Arc::new(temp_dir.path().to_path_buf()),
    };
    let routes = Router::new().route("/echo", post(ok_handler));

    // 10-byte limit; body well over it.
    let app = build_app(routes, state, 30, 10);

    let req = Request::builder()
        .method(Method::POST)
        .uri("/echo")
        .header(ORIGIN, "https://example.com")
        .header(CONTENT_LENGTH, "1000")
        .body(Body::from(vec![0u8; 1000]))?;

    let response = app.oneshot(req).await?;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(
        response.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN),
        "cross-origin clients need CORS headers to read the 413 response"
    );

    Ok(())
}

/// If `TimeoutLayer` were layered outside `CorsLayer`, its 408 response would
/// bypass CORS entirely and a cross-origin browser client couldn't read the
/// status/body. This test guards the layer ordering that prevents that.
#[tokio::test]
async fn test_request_timeout_response_has_cors_header() -> Result<()> {
    tokio::time::pause();

    let temp_dir = TempDir::new()?;
    let state = AppState {
        files_path: Arc::new(temp_dir.path().to_path_buf()),
    };
    let routes = Router::new().route("/sleep/{millis}", get(sleep_handler));

    // 1-second timeout; handler sleeps far longer (time is paused, so no real wait).
    let app = build_app(routes, state, 1, 1024 * 1024);

    let req = Request::builder()
        .uri("/sleep/9999999")
        .header(ORIGIN, "https://example.com")
        .body(Body::empty())?;
    let task = tokio::spawn(app.oneshot(req));
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(2)).await;

    let response = task.await??;

    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    assert!(
        response.headers().contains_key(ACCESS_CONTROL_ALLOW_ORIGIN),
        "cross-origin clients need CORS headers to read the 408 response"
    );

    Ok(())
}
