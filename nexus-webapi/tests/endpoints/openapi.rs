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

/// `UserDetails.epoch` is the string enum `["v0", "v1"]`.
#[test]
fn test_user_details_epoch_schema_is_a_string_enum() {
    let spec = serde_json::to_value(V0ApiDoc::merge_docs()).expect("serializable spec");
    let field = spec
        .pointer("/components/schemas/UserDetails/properties/epoch/$ref")
        .expect("UserDetails.epoch references a schema");
    assert_eq!(field, "#/components/schemas/Epoch");
    let schema = spec
        .pointer("/components/schemas/Epoch")
        .expect("Epoch schema");
    assert_eq!(schema["type"], "string", "{schema}");
    assert_eq!(schema["enum"], serde_json::json!(["v0", "v1"]), "{schema}");
}

/// Every `UserCounts` field is always present, `collections` included.
#[test]
fn test_user_counts_schema_requires_every_count() {
    let spec = serde_json::to_value(V0ApiDoc::merge_docs()).expect("serializable spec");
    let required = spec
        .pointer("/components/schemas/UserCounts/required")
        .and_then(Value::as_array)
        .expect("UserCounts lists its required fields");
    let mut required: Vec<&str> = required.iter().filter_map(Value::as_str).collect();
    required.sort_unstable();
    assert_eq!(
        required,
        [
            "bookmarks",
            "collections",
            "followers",
            "following",
            "friends",
            "posts",
            "replies",
            "tagged",
            "tags",
            "unique_tags",
        ]
    );
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

/// The `Path`, `Query` and `Json` extractors turn every rejection into a 400, so every
/// operation that takes input must list one.
#[test]
fn test_every_operation_documents_400() {
    let exempt = [
        // Takes no input.
        "get /v0/info",
        // The legacy redirect never extracts its parameters.
        "get /static/files/{owner_id}/{file_id}",
    ];

    let mut missing = Vec::new();
    for spec in [V0ApiDoc::merge_docs(), StaticApiDoc::merge_docs()] {
        let spec = serde_json::to_value(spec).expect("serializable spec");
        let paths = spec["paths"].as_object().expect("paths");
        for (path, item) in paths {
            for (method, operation) in item.as_object().expect("path item") {
                let operation_name = format!("{method} {path}");
                if operation["responses"].get("400").is_none()
                    && !exempt.contains(&operation_name.as_str())
                {
                    missing.push(operation_name);
                }
            }
        }
    }

    assert!(missing.is_empty(), "operations without a 400: {missing:?}");
}

/// `relationship` is a boolean and `tag_uri`, the viewer's tag address, a nullable string.
fn assert_relationship_and_tag_uri(schema: &str) {
    let spec = serde_json::to_value(V0ApiDoc::merge_docs()).expect("serializable spec");
    let properties = spec
        .pointer(&format!("/components/schemas/{schema}/properties"))
        .unwrap_or_else(|| panic!("{schema} properties"));
    assert_eq!(
        properties["relationship"]["type"], "boolean",
        "{schema}: {properties}"
    );
    let tag_uri = &properties["tag_uri"];
    assert_eq!(
        tag_uri["type"],
        serde_json::json!(["string", "null"]),
        "{schema}: {tag_uri}"
    );
    assert!(
        tag_uri["example"]
            .as_str()
            .is_some_and(|example| example.starts_with("pubky://")),
        "{schema}: {tag_uri}"
    );
}

#[test]
fn test_tag_details_relationship_and_tag_uri() {
    assert_relationship_and_tag_uri("TagDetails");
}

#[test]
fn test_taggers_relationship_and_tag_uri() {
    assert_relationship_and_tag_uri("TaggersInfoResponse");
}
