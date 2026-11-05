use std::collections::BTreeMap;

use nexus_common::models::post::PostKind;
use nexus_webapi::routes::{r#static::ApiDoc as StaticApiDoc, v0::ApiDoc};
use pubky_social_specs::PubkySocialPostKind;
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

/// A document has the basic structure and every schema it references.
fn assert_valid_document(spec: utoipa::openapi::OpenApi) {
    let json = serde_json::to_value(&spec).expect("Failed to serialize OpenAPI spec");

    // Basic structure checks
    assert!(json["openapi"].is_string(), "Missing openapi version");
    assert!(json["info"]["title"].is_string(), "Missing info.title");
    assert!(json["paths"].is_object(), "Missing paths");

    // Validate all $ref references are defined
    validate_openapi_refs(&json);
}

/// Test that the v0 OpenAPI spec is valid and all schema references are defined.
#[test]
fn test_v0_openapi_spec_valid() {
    assert_valid_document(ApiDoc::v0_docs());
}

/// Test that the v1 OpenAPI spec is valid and all schema references are defined.
#[test]
fn test_v1_openapi_spec_valid() {
    assert_valid_document(ApiDoc::v1_docs());
}

/// The paths of a document with their version prefix stripped, each with its operations.
fn paths_under(spec: &Value, prefix: &str) -> BTreeMap<String, Value> {
    spec["paths"]
        .as_object()
        .expect("paths")
        .iter()
        .map(|(path, item)| {
            let relative = path
                .strip_prefix(prefix)
                .unwrap_or_else(|| panic!("{path} is not under {prefix}"));
            (relative.to_string(), item.clone())
        })
        .collect()
}

/// The paths only v0 serves, relative to the prefix: the bookmark route, until R1. The tickets
/// that take a route off v1 add theirs here.
const V0_ONLY_PATHS: &[&str] = &["/post/{author_id}/{post_id}/bookmark"];

/// Both documents come from one route declaration, so every v0 path is in the v1 document
/// and vice versa, with the same operations under each, except the paths one version alone
/// serves ([`V0_ONLY_PATHS`]).
///
/// A versioned route documents its own 200 body: `PostViewDetailed` on v0 is `PostView` on
/// v1, `PostStreamDetailed` is `PostStream`. Those operations are compared without the body
/// and the body's schema name is checked on its own. A route with a kind filter describes
/// `kind` and `exclude_kinds` in its version's names: those descriptions are compared to the
/// version's names and the operations without them. The post streams describe `source` and
/// `locked`, and themselves, with the bookmark stream on v0 and without it on v1: those
/// descriptions are compared the same way.
#[test]
fn test_v0_and_v1_documents_list_the_same_paths() {
    let v0 = paths_under(&serde_json::to_value(ApiDoc::v0_docs()).unwrap(), "/v0");
    let v1 = paths_under(&serde_json::to_value(ApiDoc::v1_docs()).unwrap(), "/v1");
    assert!(!v0.is_empty(), "the v0 document lists no path");
    for &path in V0_ONLY_PATHS {
        assert!(v0.contains_key(path), "{path} missing from the v0 document");
        assert!(!v1.contains_key(path), "{path} listed in the v1 document");
    }
    let shared: Vec<_> = v0
        .keys()
        .filter(|path| !V0_ONLY_PATHS.contains(&path.as_str()))
        .collect();
    assert_eq!(shared, v1.keys().collect::<Vec<_>>());

    // Method, path and each version's 200 body schema of the routes whose body is versioned.
    let renamed_bodies = [
        (
            "get",
            "/post/{author_id}/{post_id}",
            "PostViewDetailed",
            "PostView",
        ),
        ("get", "/stream/posts", "PostStreamDetailed", "PostStream"),
        (
            "post",
            "/stream/posts/by_ids",
            "PostStreamDetailed",
            "PostStream",
        ),
    ];
    // Method, path and the kind filters of the routes that take one.
    let kind_filtered = [
        ("get", "/stream/posts", &["kind", "exclude_kinds"][..]),
        ("get", "/stream/posts/keys", &["kind", "exclude_kinds"][..]),
        ("get", "/search/posts/by_content", &["kind"][..]),
    ];
    // Method and path of the routes that describe the bookmark stream: in `source`, in
    // `locked` and in the operation's own description.
    let bookmark_sourced = [("get", "/stream/posts"), ("get", "/stream/posts/keys")];
    for (path, operations) in v0
        .iter()
        .filter(|(path, _)| !V0_ONLY_PATHS.contains(&path.as_str()))
    {
        let mut v0_operations = operations.clone();
        let mut v1_operations = v1[path].clone();
        for (method, _, v0_body, v1_body) in renamed_bodies.iter().filter(|(_, p, ..)| p == path) {
            let operation = format!("{method} {path}");
            take_200_body(&mut v0_operations[method], v0_body, &operation);
            take_200_body(&mut v1_operations[method], v1_body, &operation);
        }
        for (method, _, filters) in kind_filtered.iter().filter(|(_, p, _)| p == path) {
            let operation = format!("{method} {path}");
            for filter in *filters {
                let v0_names = take_parameter_description(&mut v0_operations[method], filter);
                assert!(
                    v0_names.contains("short, long, image, video, link, file, collection")
                        && !v0_names.contains("note"),
                    "{operation} {filter} on v0: {v0_names}"
                );
                let v1_names = take_parameter_description(&mut v1_operations[method], filter);
                assert!(
                    v1_names
                        .contains("note, article, image, video, link, file, collection, unknown")
                        && !v1_names.contains("short, long"),
                    "{operation} {filter} on v1: {v1_names}"
                );
            }
        }
        for (method, _) in bookmark_sourced.iter().filter(|(_, p)| p == path) {
            let operation = format!("{method} {path}");
            let mut v0_texts = vec![v0_operations[method]["description"].take()];
            let mut v1_texts = vec![v1_operations[method]["description"].take()];
            for parameter in ["source", "locked"] {
                v0_texts
                    .push(take_parameter_description(&mut v0_operations[method], parameter).into());
                v1_texts
                    .push(take_parameter_description(&mut v1_operations[method], parameter).into());
            }
            for (v0_text, v1_text) in v0_texts.iter().zip(&v1_texts) {
                let (v0_text, v1_text) = (v0_text.as_str().unwrap(), v1_text.as_str().unwrap());
                assert!(
                    v0_text.contains("bookmarks"),
                    "{operation} on v0: {v0_text}"
                );
                assert!(
                    !v1_text.contains("bookmark"),
                    "{operation} on v1: {v1_text}"
                );
                // The same text once the bookmark stream is written out of v0's.
                let without_bookmarks = v0_text
                    .replace("- *bookmarks*: Requires **observer_id**.\n", "")
                    .replace("bookmarks, ", "")
                    .replace(", except on `source=bookmarks`", "");
                assert_eq!(without_bookmarks, v1_text, "{operation}");
            }
        }
        assert_eq!(v0_operations, v1_operations, "{path}");
    }
}

/// Takes the description of the query parameter `name` out of `operation` and returns it.
fn take_parameter_description(operation: &mut Value, name: &str) -> String {
    let parameter = operation["parameters"]
        .as_array_mut()
        .expect("parameters")
        .iter_mut()
        .find(|parameter| parameter["name"] == name)
        .unwrap_or_else(|| panic!("no `{name}` parameter"));
    parameter["description"]
        .take()
        .as_str()
        .unwrap_or_else(|| panic!("`{name}` has no description"))
        .to_string()
}

/// Takes the 200 body's `$ref` out of `operation`, asserting it names `schema`.
fn take_200_body(operation: &mut Value, schema: &str, context: &str) {
    let reference = operation
        .pointer_mut("/responses/200/content/application~1json/schema/$ref")
        .unwrap_or_else(|| panic!("{context}: no 200 body"));
    assert_eq!(
        reference.take(),
        format!("#/components/schemas/{schema}"),
        "{context}"
    );
}

/// The post kind's names: the stored ones on v0, the `social/v1` wire names on v1, `unknown`
/// on both. The v1 list is pinned to the spec crate's names, so a renamed kind fails here and
/// not in the client; the `/v1` component keeps the v0 name, so a client reads one `PostKind`.
#[test]
fn test_post_kind_schema_names_the_kinds_per_version() {
    let kinds = |spec: utoipa::openapi::OpenApi| -> Vec<String> {
        let spec = serde_json::to_value(spec).expect("serializable spec");
        spec.pointer("/components/schemas/PostKind/enum")
            .and_then(Value::as_array)
            .expect("PostKind enum")
            .iter()
            .map(|name| name.as_str().expect("a name").to_string())
            .collect()
    };

    let v0 = kinds(ApiDoc::v0_docs());
    assert_eq!(
        v0,
        [
            "short",
            "long",
            "image",
            "video",
            "link",
            "file",
            "collection",
            "unknown"
        ]
    );

    let v1 = kinds(ApiDoc::v1_docs());
    assert_eq!(
        v1,
        [
            "note",
            "article",
            "image",
            "video",
            "link",
            "file",
            "collection",
            "unknown"
        ]
    );
    // Each v1 name is the v0 kind's `v1_name`, and the spec crate knows it under that name.
    for (v0_name, v1_name) in v0.iter().zip(&v1) {
        let stored: PostKind = serde_json::from_value(Value::from(v0_name.as_str())).unwrap();
        assert_eq!(stored.v1_name(), v1_name);
        let spec = PubkySocialPostKind::from(v1_name.clone());
        assert_eq!(spec.is_known(), v1_name != "unknown", "{v1_name}");
        assert_eq!(spec.wire_name(), v1_name, "{v1_name}");
    }
}

/// The v1 schemas keep `epoch`, `tag_uri` and `deleted` where v0 has them.
#[test]
fn test_v1_schemas_keep_epoch_tag_uri_and_deleted() {
    let spec = serde_json::to_value(ApiDoc::v1_docs()).expect("serializable spec");
    for (schema, field) in [
        ("UserDetails", "epoch"),
        ("TagDetails", "tag_uri"),
        ("TaggersInfoResponse", "tag_uri"),
        ("PostDetails", "deleted"),
    ] {
        let object = spec
            .pointer(&format!("/components/schemas/{schema}"))
            .unwrap_or_else(|| panic!("{schema} missing from the v1 document"));
        assert!(
            object["properties"].get(field).is_some(),
            "{schema}.{field}: {object}"
        );
    }
    // The v1 post view is the v1 details, kind included.
    assert_eq!(
        spec.pointer("/components/schemas/PostView/properties/details/$ref"),
        Some(&Value::from("#/components/schemas/PostDetails"))
    );
    assert_eq!(
        spec.pointer("/components/schemas/PostDetails/properties/kind/$ref"),
        Some(&Value::from("#/components/schemas/PostKind"))
    );
    assert_eq!(
        spec.pointer("/components/schemas/PostStream/items/$ref"),
        Some(&Value::from("#/components/schemas/PostView"))
    );
}

/// The `exclude_kinds` list's schema is a string on both versions; the v1 one says `unknown`
/// is a name there.
#[test]
fn test_post_kinds_schema_per_version() {
    for (spec, unknown_listed) in [(ApiDoc::v0_docs(), false), (ApiDoc::v1_docs(), true)] {
        let spec = serde_json::to_value(spec).expect("serializable spec");
        let schema = spec
            .pointer("/components/schemas/PostKinds")
            .expect("PostKinds schema");
        assert_eq!(schema["type"], "string", "{schema}");
        let description = schema["description"].as_str().expect("description");
        assert_eq!(
            description.contains("`unknown`"),
            unknown_listed,
            "{description}"
        );
    }
}

/// `StreamReach` is a query string like `wot_2`, not the Rust enum shape.
#[test]
fn test_stream_reach_schema_is_its_string_form() {
    let spec = serde_json::to_value(ApiDoc::v0_docs()).expect("serializable spec");
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
    let spec = serde_json::to_value(ApiDoc::v0_docs()).expect("serializable spec");
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

/// Post kinds and profile links are documented under Nexus's names, with the stored kind names.
#[test]
fn test_post_kind_and_user_link_schemas_are_nexus_owned() {
    let spec = serde_json::to_value(ApiDoc::v0_docs()).expect("serializable spec");
    let schemas = spec
        .pointer("/components/schemas")
        .and_then(Value::as_object)
        .expect("schemas");
    for gone in ["PubkyAppPostKind", "PubkyAppUserLink"] {
        assert!(!schemas.contains_key(gone), "{gone} is still documented");
    }
    assert!(schemas.contains_key("UserLink"));
    assert_eq!(
        spec.pointer("/components/schemas/PostDetails/properties/kind/$ref"),
        Some(&Value::from("#/components/schemas/PostKind"))
    );
    assert_eq!(
        schemas["PostKind"]["enum"],
        serde_json::json!([
            "short",
            "long",
            "image",
            "video",
            "link",
            "file",
            "collection",
            "unknown"
        ])
    );
}

/// `attachments` and `links` are always present lists, never `null`.
#[test]
fn test_attachments_and_links_are_required_lists() {
    let spec = serde_json::to_value(ApiDoc::v0_docs()).expect("serializable spec");
    for (schema, field) in [("PostDetails", "attachments"), ("UserDetails", "links")] {
        let object = spec
            .pointer(&format!("/components/schemas/{schema}"))
            .expect("schema");
        let required = object["required"].as_array().expect("required fields");
        assert!(required.contains(&Value::from(field)), "{schema}.{field}");
        assert_eq!(
            object["properties"][field]["type"], "array",
            "{schema}.{field}"
        );
    }
}

/// Every `UserCounts` field is always present, `collections` included; `bookmarks` is one of
/// them on v0 and absent on v1.
#[test]
fn test_user_counts_schema_requires_every_count() {
    let counts_without_bookmarks = [
        "collections",
        "followers",
        "following",
        "friends",
        "posts",
        "replies",
        "tagged",
        "tags",
        "unique_tags",
    ];
    for (spec, with_bookmarks) in [(ApiDoc::v0_docs(), true), (ApiDoc::v1_docs(), false)] {
        let spec = serde_json::to_value(spec).expect("serializable spec");
        let schema = spec
            .pointer("/components/schemas/UserCounts")
            .expect("UserCounts schema");
        let mut required: Vec<&str> = schema["required"]
            .as_array()
            .expect("UserCounts lists its required fields")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        required.sort_unstable();
        let mut expected = counts_without_bookmarks.to_vec();
        if with_bookmarks {
            expected.insert(0, "bookmarks");
        }
        assert_eq!(required, expected, "{schema}");
        assert_eq!(
            schema["properties"].get("bookmarks").is_some(),
            with_bookmarks,
            "{schema}"
        );
    }
}

/// The schema `pointer` references, resolved in `spec`.
fn referenced<'a>(spec: &'a Value, pointer: &str) -> &'a Value {
    let reference = spec
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{pointer} is not a $ref in {spec}"));
    let name = reference
        .strip_prefix("#/components/schemas/")
        .unwrap_or_else(|| panic!("{pointer}: {reference}"));
    spec.pointer(&format!("/components/schemas/{name}"))
        .unwrap_or_else(|| panic!("{reference} is not defined"))
}

/// The user view, the user stream and the bootstrap embed the version's counts: the `counts`
/// of `UserView` resolve to a schema with `bookmarks` on v0 and without on v1, the stream
/// lists that view, and the bootstrap's `users` are that stream. On v1 the bootstrap's posts
/// and notifications are the v1 ones too, so no v1 schema has a `bookmark` or `bookmarks`
/// property and the `Bookmark` component is v0's alone.
#[test]
fn test_user_view_schema_embeds_the_versions_counts() {
    for (spec, with_bookmarks) in [(ApiDoc::v0_docs(), true), (ApiDoc::v1_docs(), false)] {
        let spec = serde_json::to_value(spec).expect("serializable spec");
        let counts = referenced(&spec, "/components/schemas/UserView/properties/counts/$ref");
        assert_eq!(
            counts["properties"].get("bookmarks").is_some(),
            with_bookmarks,
            "{counts}"
        );
        assert!(counts["properties"].get("followers").is_some(), "{counts}");
        for field in ["details", "tags", "relationship", "social_graph_status"] {
            assert!(
                spec.pointer(&format!("/components/schemas/UserView/properties/{field}"))
                    .is_some(),
                "UserView.{field}"
            );
        }
        assert_eq!(
            spec.pointer("/components/schemas/UserStream/items/$ref"),
            Some(&Value::from("#/components/schemas/UserView"))
        );
        assert_eq!(
            spec.pointer("/components/schemas/Bootstrap/properties/users/$ref"),
            Some(&Value::from("#/components/schemas/UserStream"))
        );
        assert_eq!(
            spec.pointer("/components/schemas/Bootstrap/properties/posts/$ref"),
            Some(&Value::from("#/components/schemas/PostStream"))
        );
        assert_eq!(
            spec.pointer("/components/schemas/Bootstrap/properties/notifications/items/$ref"),
            Some(&Value::from("#/components/schemas/Notification"))
        );
        // The bootstrap's posts: the stored `PostView` on v0, the v1 one on v1.
        let post = referenced(&spec, "/components/schemas/PostStream/items/$ref");
        assert_eq!(
            post["properties"].get("bookmark").is_some(),
            with_bookmarks,
            "{post}"
        );

        let schemas = spec
            .pointer("/components/schemas")
            .and_then(Value::as_object)
            .expect("schemas");
        let with_bookmark_fields: Vec<&String> = schemas
            .iter()
            .filter(|(_, schema)| {
                schema["properties"].as_object().is_some_and(|fields| {
                    fields.contains_key("bookmark") || fields.contains_key("bookmarks")
                })
            })
            .map(|(name, _)| name)
            .collect();
        if with_bookmarks {
            assert!(schemas.contains_key("Bookmark"), "{with_bookmark_fields:?}");
            assert!(!with_bookmark_fields.is_empty());
        } else {
            assert!(!schemas.contains_key("Bookmark"));
            assert_eq!(with_bookmark_fields, Vec::<&String>::new());
        }
    }
}

/// Bookmarks are v0's alone: `social/v1` keeps them under `/priv/`, which Nexus never sees.
/// The v1 post view has no `bookmark`, the v1 stream sources no `bookmarks`, and the bookmark
/// route is in the v0 document only; v0 keeps all three.
#[test]
fn test_bookmarks_are_documented_on_v0_only() {
    let v0 = serde_json::to_value(ApiDoc::v0_docs()).expect("serializable spec");
    let v1 = serde_json::to_value(ApiDoc::v1_docs()).expect("serializable spec");

    // v0's `PostViewDetailed` extends the stored `PostView` through `allOf`; the fields are there.
    assert_eq!(
        v0.pointer("/components/schemas/PostViewDetailed/allOf/0/$ref"),
        Some(&Value::from("#/components/schemas/PostView"))
    );
    let v0_view = v0
        .pointer("/components/schemas/PostView/properties")
        .expect("PostView properties");
    assert!(v0_view.get("bookmark").is_some(), "{v0_view}");
    let v1_view = v1
        .pointer("/components/schemas/PostView/properties")
        .expect("PostView properties");
    assert!(v1_view.get("bookmark").is_none(), "{v1_view}");
    for field in ["details", "counts", "tags", "relationships"] {
        assert!(v1_view.get(field).is_some(), "PostView.{field}: {v1_view}");
    }

    let sources = |spec: &Value| -> Vec<String> {
        spec.pointer("/components/schemas/StreamSourceKind/enum")
            .and_then(Value::as_array)
            .expect("StreamSourceKind enum")
            .iter()
            .map(|name| name.as_str().expect("a name").to_string())
            .collect()
    };
    let v0_sources = sources(&v0);
    let v1_sources = sources(&v1);
    assert!(
        v0_sources.iter().any(|name| name == "bookmarks"),
        "{v0_sources:?}"
    );
    let v0_without_bookmarks: Vec<&String> = v0_sources
        .iter()
        .filter(|name| *name != "bookmarks")
        .collect();
    assert_eq!(v1_sources.iter().collect::<Vec<_>>(), v0_without_bookmarks);

    let bookmark_route = "/post/{author_id}/{post_id}/bookmark";
    assert!(
        v0["paths"].get(format!("/v0{bookmark_route}")).is_some(),
        "the bookmark route is missing from the v0 document"
    );
    assert!(
        v1["paths"].get(format!("/v1{bookmark_route}")).is_none(),
        "the bookmark route is listed in the v1 document"
    );
    assert!(
        v1["paths"]
            .as_object()
            .expect("paths")
            .keys()
            .all(|path| !path.contains("bookmark")),
        "a bookmark path is listed in the v1 document"
    );
}

/// Test that the static OpenAPI spec is valid and all schema references are defined.
#[test]
fn test_static_openapi_spec_valid() {
    assert_valid_document(StaticApiDoc::merge_docs());
}

/// The v1 files route sits in the static document, next to the v0 route it extends, until
/// API1 moves it to the v1 one; a missing v1 file is a 404 there, so the route lists one.
#[test]
fn test_static_openapi_lists_both_files_routes() {
    let spec = serde_json::to_value(StaticApiDoc::merge_docs()).expect("serializable spec");
    let paths = spec["paths"].as_object().expect("paths");
    for path in [
        "/static/files/{owner_id}/{file_id}/{variant}",
        "/v1/static/files/{owner_id}/{file_id}/{variant}",
    ] {
        assert!(
            paths.contains_key(path),
            "{path} missing from {:?}",
            paths.keys().collect::<Vec<_>>()
        );
    }
    let v1 = &spec["paths"]["/v1/static/files/{owner_id}/{file_id}/{variant}"]["get"];
    assert!(v1["responses"].get("404").is_some(), "{v1}");
    assert!(v1["responses"].get("400").is_some(), "{v1}");
}

/// The `Path`, `Query` and `Json` extractors turn every rejection into a 400, so every
/// operation that takes input must list one.
#[test]
fn test_every_operation_documents_400() {
    let exempt = [
        // Takes no input.
        "get /v0/info",
        "get /v1/info",
        // The legacy redirect never extracts its parameters.
        "get /static/files/{owner_id}/{file_id}",
    ];

    let mut missing = Vec::new();
    for spec in [
        ApiDoc::v0_docs(),
        ApiDoc::v1_docs(),
        StaticApiDoc::merge_docs(),
    ] {
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
    let spec = serde_json::to_value(ApiDoc::v0_docs()).expect("serializable spec");
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

/// A resource tag keeps no `app`, so the resource stream routes take no `app` parameter.
#[test]
fn test_resource_stream_routes_take_no_app_parameter() {
    let spec = serde_json::to_value(ApiDoc::v0_docs()).expect("serializable spec");
    for path in ["/v0/stream/resources", "/v0/stream/resources/ids"] {
        let params = spec["paths"][path]["get"]["parameters"]
            .as_array()
            .unwrap_or_else(|| panic!("{path} parameters"));
        let names: Vec<_> = params.iter().filter_map(|p| p["name"].as_str()).collect();
        assert!(names.contains(&"tags"), "{path}: {names:?}");
        assert!(!names.contains(&"app"), "{path}: {names:?}");
    }
}

/// The avatar is one handler mounted twice: under `/static` for v0 clients and under the v1
/// base path, so a v1 client reads an avatar where it reads every other file. Both mounts sit in
/// the static document until API1 moves the v1 one, and each lists the 404 a client falls back
/// from when the profile image is not an image file Nexus holds.
#[test]
fn test_static_openapi_lists_both_avatar_mounts() {
    let spec = serde_json::to_value(StaticApiDoc::merge_docs()).expect("serializable spec");
    let paths = spec["paths"].as_object().expect("paths");
    for path in ["/static/avatar/{user_id}", "/v1/static/avatar/{user_id}"] {
        let item = paths.get(path).unwrap_or_else(|| {
            panic!("{path} missing from {:?}", paths.keys().collect::<Vec<_>>())
        });
        let operation = &item["get"];
        assert!(
            operation["responses"].get("404").is_some(),
            "{path}: {operation}"
        );
    }
}
