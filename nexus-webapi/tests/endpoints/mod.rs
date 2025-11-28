use crate::utils::{host_url, server::TestServiceServer};

use anyhow::Result;
use axum::http::Method;
use nexus_webapi::routes::{r#static::ApiDoc as StaticApiDoc, v0::ApiDoc as V0ApiDoc};
use serde_json::Value;

#[tokio_shared_rt::test(shared)]
async fn test_swagger_ui() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let res = client.do_get("/swagger-ui").await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("<html"));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_openapi_schema() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let res = client.do_get("/api-docs/v0/openapi.json").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["openapi"].is_string());
    assert!(body["info"]["title"].is_string());
    assert_eq!(body["info"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["paths"].is_object());

    Ok(())
}

/// Validates that all schema references in an OpenAPI spec are defined.
fn validate_openapi_refs(spec: &Value) {
    let schemas = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object);

    let missing: Vec<_> = collect_refs(spec)
        .filter(|r| r.starts_with("#/components/schemas/"))
        .filter_map(|r| r.strip_prefix("#/components/schemas/"))
        .filter(|name| schemas.is_none_or(|s| !s.contains_key(*name)))
        .collect();

    assert!(missing.is_empty(), "Missing schemas: {missing:?}");
}

/// Recursively collects all `$ref` values from a JSON value.
fn collect_refs(value: &Value) -> Box<dyn Iterator<Item = &str> + '_> {
    match value {
        Value::Object(map) => {
            let ref_value = map.get("$ref").and_then(Value::as_str);
            let children = map.values().flat_map(collect_refs);
            Box::new(ref_value.into_iter().chain(children))
        }
        Value::Array(arr) => Box::new(arr.iter().flat_map(collect_refs)),
        _ => Box::new(std::iter::empty()),
    }
}

/// Test that the v0 OpenAPI spec is valid and all schema references are defined.
#[test]
fn test_v0_openapi_spec_valid() {
    let spec = V0ApiDoc::merge_docs();
    let json = serde_json::to_value(&spec).expect("Failed to serialize v0 OpenAPI spec");

    // Basic structure checks
    assert!(json["openapi"].is_string(), "Missing openapi version");
    assert!(json["info"]["title"].is_string(), "Missing info.title");
    assert!(json["paths"].is_object(), "Missing paths");

    // Validate all $ref references are defined
    validate_openapi_refs(&json);
}

/// Test that the static OpenAPI spec is valid and all schema references are defined.
#[test]
fn test_static_openapi_spec_valid() {
    let spec = StaticApiDoc::merge_docs();
    let json = serde_json::to_value(&spec).expect("Failed to serialize static OpenAPI spec");

    // Basic structure checks
    assert!(json["openapi"].is_string(), "Missing openapi version");
    assert!(json["info"]["title"].is_string(), "Missing info.title");
    assert!(json["paths"].is_object(), "Missing paths");

    // Validate all $ref references are defined
    validate_openapi_refs(&json);
}

#[tokio_shared_rt::test(shared)]
async fn test_info_endpoint() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let res = client.do_get("/v0/info").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    println!("body: {body:?}");
    assert_eq!(body["name"], env!("CARGO_PKG_NAME"));
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_pkarr_endpoint() -> Result<()> {
    let test_server = TestServiceServer::get_test_server().await;
    let pubky_tls_dns_url = test_server.nexus_api.pubky_tls_dns_url();

    let sdk = test_server.testnet.sdk()?;
    let response = sdk
        .client()
        .request(Method::GET, &format!("{pubky_tls_dns_url}/v0/info"))
        .send()
        .await?;

    assert_eq!(response.status(), 200);

    Ok(())
}
