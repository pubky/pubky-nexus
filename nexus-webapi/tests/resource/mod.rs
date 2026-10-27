mod references;
use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::{json, Value};

// Resource IDs from docker/test-graph/mocks/resources.cypher
// Computed as hex(BLAKE3(normalized_uri)[0..16])
const RESOURCE_1_ID: &str = "450a72e3da164bfc3ac5f4056f9e5c7c"; // https://example.com/article
const RESOURCE_2_ID: &str = "fb4155a2295ff3a8a8fe02e28229c021"; // pubky://somepk/.../events/E001

// Resource 1 taggers: both tagged "bitcoin", only Amsterdam tagged "interesting"
const AMSTERDAM: &str = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";
const BOGOTA: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";
const AMSTERDAM_BITCOIN_TAG_URI: &str =
    "pubky://emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy/pub/mapky/tags/RES_TAG_001";
const BOGOTA_BITCOIN_TAG_URI: &str =
    "pubky://ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny/pub/mapky/tags/RES_TAG_002";

// From docker/test-graph/mocks/tag-relationship.cypher. On the first resource two users
// tagged "trel-shared" and one tagged "trel-other". On the second, the viewer tagged
// "trel-shared" from two apps and another user tagged "trel-other" once
const RANKED_RESOURCE_ID: &str = "7a6e1c0de5f0a11ce0ffee0000000001";
const TWO_APP_RESOURCE_ID: &str = "7a6e1c0de5f0a11ce0ffee0000000002";
const TWO_APP_VIEWER: &str = "xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o";

// =============================================
// GET /v0/resource/:resource_id/tags
// =============================================

#[tokio_shared_rt::test(shared)]
async fn test_resource_tags() -> Result<()> {
    let path = format!("/v0/resource/{RESOURCE_1_ID}/tags");
    let body = get_request(&path).await?;

    // Response is now {resource, tags} envelope
    assert!(body["resource"].is_object(), "Should have resource object");
    assert!(body["tags"].is_array(), "Should have tags array");

    // Verify resource metadata
    assert_eq!(body["resource"]["id"], RESOURCE_1_ID);
    assert_eq!(body["resource"]["scheme"], "https");
    assert!(body["resource"]["uri"].is_string());

    let tags = body["tags"].as_array().expect("Should be array");
    // Resource 1 has 2 labels: "bitcoin" (2 taggers) and "interesting" (1 tagger)
    assert_eq!(
        tags.len(),
        2,
        "Resource 1 should have 2 distinct tag labels"
    );

    // Verify tag structure
    for tag in tags {
        assert!(tag["label"].is_string(), "label should be a string");
        assert!(tag["taggers"].is_array(), "taggers should be an array");
        assert!(
            tag["taggers_count"].is_number(),
            "taggers_count should be a number"
        );
    }

    // Find the "bitcoin" tag — should have 2 taggers
    let bitcoin_tag = tags.iter().find(|t| t["label"] == "bitcoin");
    assert!(bitcoin_tag.is_some(), "Should have bitcoin tag");
    assert_eq!(bitcoin_tag.unwrap()["taggers_count"], 2);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_tags_pagination_and_viewer() -> Result<()> {
    let path = format!("/v0/resource/{RESOURCE_1_ID}/tags");

    // Labels are ranked by tagger count and paginated
    let body = get_request(&format!("{path}?limit_tags=1")).await?;
    assert_eq!(labels(&body), ["bitcoin"]);
    let body = get_request(&format!("{path}?skip_tags=1")).await?;
    assert_eq!(labels(&body), ["interesting"]);
    // The label with more taggers comes first, even when it sorts after by label
    let body = get_request(&format!("/v0/resource/{RANKED_RESOURCE_ID}/tags")).await?;
    assert_eq!(labels(&body), ["trel-shared", "trel-other"]);

    // The tagger preview is capped, the count is not, and the viewer is flagged,
    // with their tag's address, against all of a label's taggers, not only the
    // previewed one
    for (viewer, viewer_uri) in [
        (AMSTERDAM, AMSTERDAM_BITCOIN_TAG_URI),
        (BOGOTA, BOGOTA_BITCOIN_TAG_URI),
    ] {
        let body = get_request(&format!("{path}?limit_taggers=1&viewer_id={viewer}")).await?;
        let bitcoin = &body["tags"][0];
        assert_eq!(bitcoin["label"], "bitcoin");
        assert_eq!(bitcoin["taggers"].as_array().map(Vec::len), Some(1));
        assert_eq!(bitcoin["taggers_count"], 2);
        assert_eq!(bitcoin["relationship"], true, "{viewer} tagged bitcoin");
        assert_eq!(bitcoin["tag_uri"], viewer_uri);
    }

    // Only the labels the viewer applied are flagged
    let body = get_request(&format!("{path}?viewer_id={BOGOTA}")).await?;
    assert_eq!(labels(&body), ["bitcoin", "interesting"]);
    assert_eq!(body["tags"][0]["relationship"], true);
    assert_eq!(body["tags"][1]["relationship"], false);
    assert_eq!(body["tags"][1]["tag_uri"], Value::Null);

    Ok(())
}

/// A user who tagged a label from two apps counts once: both labels have one tagger, so
/// they tie and come back in label order, and the taggers list the user once.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tags_rank_distinct_taggers() -> Result<()> {
    let path = format!("/v0/resource/{TWO_APP_RESOURCE_ID}/tags");
    let body = get_request(&path).await?;
    assert_eq!(labels(&body), ["trel-other", "trel-shared"]);
    let shared = &body["tags"][1];
    assert_eq!(shared["taggers_count"], 1);
    assert_eq!(shared["taggers"], json!([TWO_APP_VIEWER]));

    let body = get_request(&format!("{path}/trel-shared/taggers")).await?;
    assert_eq!(body["users"], json!([TWO_APP_VIEWER]));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_tags_not_found() -> Result<()> {
    // Non-existent resource (valid hex but no data)
    let path = "/v0/resource/00000000000000000000000000000000/tags";
    invalid_get_request(path, StatusCode::NOT_FOUND).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_tags_invalid_id() -> Result<()> {
    // Invalid resource_id format (not 32-char hex)
    let path = "/v0/resource/invalid-id/tags";
    invalid_get_request(path, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

// =============================================
// GET /v0/resource/by-uri
// =============================================

#[tokio_shared_rt::test(shared)]
async fn test_resource_by_uri() -> Result<()> {
    let uri = "https%3A%2F%2Fexample.com%2Farticle";
    let path = format!("/v0/resource/by-uri?uri={uri}");
    let body = get_request(&path).await?;

    // Response is {resource, tags} envelope
    assert!(body["resource"].is_object(), "Should have resource object");
    assert!(body["tags"].is_array(), "Should have tags array");
    let tags = body["tags"].as_array().expect("Should be array");
    assert_eq!(tags.len(), 2, "Article resource should have 2 tag labels");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_by_uri_not_found() -> Result<()> {
    let uri = "https%3A%2F%2Fnonexistent.example.com%2Fnothing";
    let path = format!("/v0/resource/by-uri?uri={uri}");
    invalid_get_request(&path, StatusCode::NOT_FOUND).await?;
    Ok(())
}

// =============================================
// GET /v0/resource/:resource_id/tags/:label/taggers
// =============================================

#[tokio_shared_rt::test(shared)]
async fn test_resource_taggers() -> Result<()> {
    let path = format!("/v0/resource/{RESOURCE_1_ID}/tags/bitcoin/taggers");
    let body = get_request(&path).await?;

    // Read from the graph and sorted by user id
    assert_eq!(body["users"], json!([AMSTERDAM, BOGOTA]));
    assert_eq!(
        (&body["relationship"], &body["tag_uri"]),
        (&json!(false), &Value::Null),
        "Should be untagged without a viewer"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_taggers_pagination_and_viewer() -> Result<()> {
    let path = format!("/v0/resource/{RESOURCE_1_ID}/tags/bitcoin/taggers");

    let body = get_request(&format!("{path}?limit=1")).await?;
    assert_eq!(body["users"], json!([AMSTERDAM]));

    // The viewer's tag comes with every page, not only the one listing them
    let body = get_request(&format!("{path}?skip=1&viewer_id={AMSTERDAM}")).await?;
    assert_eq!(body["users"], json!([BOGOTA]));
    assert_eq!(
        (&body["relationship"], &body["tag_uri"]),
        (&json!(true), &json!(AMSTERDAM_BITCOIN_TAG_URI))
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_taggers_empty_label() -> Result<()> {
    // Label that no one has used
    let path = format!("/v0/resource/{RESOURCE_1_ID}/tags/nonexistent-label/taggers");
    let body = get_request(&path).await?;

    assert!(body["users"].is_array());
    let users = body["users"].as_array().expect("users should be array");
    assert_eq!(users.len(), 0, "Non-existent label should have 0 taggers");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_resource_taggers_structure() -> Result<()> {
    let path = format!("/v0/resource/{RESOURCE_2_ID}/tags/calendar/taggers");
    let body = get_request(&path).await?;

    assert_eq!(body["users"], json!([BOGOTA]));
    assert_eq!(
        (&body["relationship"], &body["tag_uri"]),
        (&json!(false), &Value::Null),
        "Should be untagged without a viewer"
    );

    Ok(())
}

fn labels(body: &Value) -> Vec<&str> {
    body["tags"]
        .as_array()
        .expect("tags should be an array")
        .iter()
        .filter_map(|tag| tag["label"].as_str())
        .collect()
}
