//! API behaviour in light mode (`mode = "light"`).
//!
//! The shared test server runs in full mode, and the mode is process-wide, so these tests
//! set up a light stack themselves and drive a standalone router. They rely on nextest
//! running every test in its own process. The router reads the full mock data, which is
//! what a light API serves too: it leaves out the content it does not keep.

mod files;
mod users;

use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_common::{NexusMode, RateLimitConfig, StackConfig, StackManager};
use nexus_webapi::media::test_utils::default_subprocess_tests;
use nexus_webapi::media::MediaPermits;
use nexus_webapi::routes::{app_routes, build_app, AppState};
use serde_json::Value;
use tokio::sync::watch;
use tower::ServiceExt;

/// Sets up a light stack. Panics if this process already set up a full one.
pub async fn setup_light_stack() -> StackConfig {
    let stack = StackConfig {
        mode: NexusMode::Light,
        ..StackConfig::default()
    };
    StackManager::setup(&stack)
        .await
        .expect("set up a light stack; light tests need a process of their own");
    stack
}

/// A router over a light stack.
pub async fn light_app() -> Router {
    let stack = setup_light_stack().await;

    let state = AppState::new(
        stack.files_path.clone(),
        default_ingestor_tests(),
        MediaPermits::new(1),
        default_subprocess_tests(),
    );
    let (_tx, rx) = watch::channel(false);
    let routes = app_routes(state.clone(), &RateLimitConfig::default(), rx);
    build_app(routes, state, 30, 10 * 1024 * 1024)
}

/// Sends a request to a light router; returns the status and the JSON body (`Null` when
/// the body is not JSON).
pub async fn light_request(
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> Result<(StatusCode, Value)> {
    let request = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(json) => request
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&json)?))?,
        None => request.body(Body::empty())?,
    };
    let response = light_app().await.oneshot(request).await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Ok((status, json))
}

/// Asserts the light-mode answer of an endpoint a light Nexus does not serve.
pub fn assert_unavailable(status: StatusCode, body: &Value, uri: &str) {
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{uri}: status");
    assert_eq!(body["error"], "unavailable in light mode", "{uri}: body");
}

#[tokio_shared_rt::test(shared)]
async fn test_light_info_reports_the_mode() -> Result<()> {
    let (status, body) = light_request(Method::GET, "/v0/info", None).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mode"], "light");
    Ok(())
}
