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
/// This catches issues where parameters reference schemas that don't exist.
fn validate_openapi_refs(spec: &Value) -> Result<(), String> {
    let schemas = spec
        .get("components")
        .and_then(|c| c.get("schemas"))
        .cloned()
        .unwrap_or(Value::Object(serde_json::Map::new()));

    let mut errors = Vec::new();

    // Recursively find all $ref values and check they exist
    fn find_refs(value: &Value, refs: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(ref_path)) = map.get("$ref") {
                    refs.push(ref_path.clone());
                }
                for v in map.values() {
                    find_refs(v, refs);
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    find_refs(v, refs);
                }
            }
            _ => {}
        }
    }

    let mut refs = Vec::new();
    find_refs(spec, &mut refs);

    for ref_path in refs {
        // Parse refs like "#/components/schemas/SortOrder"
        if ref_path.starts_with("#/components/schemas/") {
            let schema_name = ref_path.strip_prefix("#/components/schemas/").unwrap();
            if schemas.get(schema_name).is_none() {
                errors.push(format!("Missing schema: {}", schema_name));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
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
    if let Err(e) = validate_openapi_refs(&json) {
        panic!("v0 OpenAPI spec has invalid references:\n{}", e);
    }
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
    if let Err(e) = validate_openapi_refs(&json) {
        panic!("static OpenAPI spec has invalid references:\n{}", e);
    }
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
