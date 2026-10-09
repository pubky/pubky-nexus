use nexus_webapi::routes::{r#static::ApiDoc as StaticApiDoc, v0::ApiDoc as V0ApiDoc};
use serde_json::Value;

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

/// `StreamReach` is a query string like `wot_2`, not the Rust enum shape.
#[test]
fn test_stream_reach_schema_is_its_string_form() {
    let spec = serde_json::to_value(V0ApiDoc::merge_docs()).expect("serializable spec");
    let schema = spec
        .pointer("/components/schemas/StreamReach")
        .expect("StreamReach schema");
    assert_eq!(schema["type"], "string", "{schema}");
    let values = schema["enum"].as_array().expect("enum values");
    assert!(values.contains(&Value::from("wot_2")), "{schema}");
    assert!(values.contains(&Value::from("followers")), "{schema}");
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

/// Clients learn how to use a light Nexus from the spec: the guide in its description, the
/// `mode` in `/v0/info`, and the 501 on every endpoint a light Nexus does not serve.
#[test]
fn test_v0_openapi_spec_documents_light_mode() {
    let spec = serde_json::to_value(V0ApiDoc::merge_docs()).expect("serializable spec");
    let description = spec["info"]["description"].as_str().expect("a description");
    assert!(description.contains("## Light mode"), "{description}");
    assert_eq!(
        spec.pointer("/components/schemas/NexusMode/enum"),
        Some(&serde_json::json!(["full", "light"]))
    );

    let static_spec = serde_json::to_value(StaticApiDoc::merge_docs()).expect("serializable");
    let unavailable = [
        (&spec, "/v0/search/posts/by_content"),
        (&spec, "/v0/search/users/by_name/{prefix}"),
        (&spec, "/v0/stream/users/username"),
        (&spec, "/v0/stream/posts"),
        (&spec, "/v0/stream/posts/keys"),
        (&static_spec, "/static/avatar/{user_id}"),
        (&static_spec, "/static/files/{owner_id}/{file_id}"),
        (&static_spec, "/static/files/{owner_id}/{file_id}/{variant}"),
    ];
    for (spec, path) in unavailable {
        assert!(
            spec["paths"][path]["get"]["responses"]["501"].is_object(),
            "{path} documents no 501"
        );
    }
}
