use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use nexus_common::RateLimitConfig;
use nexus_webapi::routes::middlewares::rate_limit::{
    apply_rate_limit_default, apply_rate_limit_expensive,
};
use tower::ServiceExt;

// ── Flood test ─────────────────────────────────────────────────────────

/// Build a tiny router with an expensive rate-limit layer (burst=3)
/// and a single handler that returns "ok".
fn build_test_router() -> (Router, tokio::sync::watch::Receiver<bool>) {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let config = RateLimitConfig {
        enabled: true,
        trust_proxy_headers: true,
        expensive_bucket: nexus_common::RateLimitBucketConfig {
            rate: 20, // 20 req/min
            burst: 3, // burst of 3
        },
        ..Default::default()
    };

    let app = Router::new().route("/", get(|| async { "ok" }));
    let app = apply_rate_limit_expensive(app, &config, shutdown_rx.clone());

    drop(shutdown_tx);
    (app, shutdown_rx)
}

#[tokio::test]
async fn flood_test_expensive_bucket_returns_429_after_burst() {
    let (router, _shutdown_rx) = build_test_router();

    let mut ok_count = 0;
    let mut rejected = false;
    let mut retry_after_header = None;

    // Fire 5 requests — burst is 3, so first 3 should pass, 4th should 429
    for i in 0..5 {
        let request = Request::builder()
            .uri("/")
            .header("x-forwarded-for", "127.0.0.1")
            .body(Body::empty())
            .unwrap();
        let response = router.clone().oneshot(request).await.unwrap();

        if response.status() == StatusCode::OK {
            ok_count += 1;
        } else if response.status() == StatusCode::TOO_MANY_REQUESTS {
            rejected = true;
            if let Some(header_val) = response.headers().get("Retry-After") {
                retry_after_header = Some(
                    header_val
                        .to_str()
                        .expect("valid header")
                        .parse::<u64>()
                        .expect("valid retry-after"),
                );
            }
        } else {
            panic!("Unexpected status {:?} on request {}", response.status(), i);
        }
    }

    assert_eq!(
        ok_count, 3,
        "Expected exactly 3 requests to pass within burst"
    );
    assert!(rejected, "Expected at least one 429 after burst exhausted");
    assert!(
        retry_after_header.is_some(),
        "Expected Retry-After header on 429 response"
    );
    assert!(
        retry_after_header.unwrap() >= 1,
        "Retry-After should be ≥ 1"
    );
}

#[tokio::test]
async fn rate_limit_disabled_allows_all_requests() {
    let config = RateLimitConfig {
        enabled: false,
        ..Default::default()
    };

    let (_, shutdown_rx) = tokio::sync::watch::channel(false);
    let base = Router::new().route("/", get(|| async { "ok" }));

    // When disabled the router comes back unchanged (no layer applied)
    let with_expensive = apply_rate_limit_expensive(base.clone(), &config, shutdown_rx.clone());
    let with_default = apply_rate_limit_default(base, &config, shutdown_rx);

    for router in [with_expensive, with_default] {
        let response = router
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
