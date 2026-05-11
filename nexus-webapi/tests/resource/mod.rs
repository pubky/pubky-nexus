use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;

// Resource IDs from docker/test-graph/mocks/resources.cypher
// Computed as hex(BLAKE3(normalized_uri)[0..16])
const RESOURCE_1_ID: &str = "450a72e3da164bfc3ac5f4056f9e5c7c"; // https://example.com/article
const RESOURCE_2_ID: &str = "fb4155a2295ff3a8a8fe02e28229c021"; // pubky://somepk/.../events/E001

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

    // TaggersCollection is Redis-only (no graph fallback), so with Neo4j seed data
    // the tagger sets are empty. Verify the endpoint returns valid structure.
    assert!(body["users"].is_array(), "Should have users array");
    assert!(
        body["relationship"].is_boolean(),
        "Should have relationship field"
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

    // Verify response structure (Redis cache may be empty for seed data)
    assert!(body["users"].is_array(), "Should have users array");
    assert!(
        body["relationship"].is_boolean(),
        "Should have relationship bool"
    );

    Ok(())
}
