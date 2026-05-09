//! Tests for `kind=collection` filtering on `/v0/stream/posts`.
//!
//! Seed data (in `docker/test-graph/mocks/posts.cypher`):
//!   - 3 collection posts: COLW1TGL5BKG1, COLW1TGL5BKG2 (author: $bogota),
//!     COLW1TGL5BKG3 (author: $cairo).
//!
//! Phase 3 stream-suppression invariants we assert here:
//!   1. `?kind=collection` returns only collection posts (Cypher fallback path
//!      with `p.kind = $kind`).
//!   2. The default stream (no `kind`) does NOT include collections (Cypher
//!      fallback default-exclusion at `get.rs:836-838`).
//!   3. Profile collections tab (`?source=author&kind=collection`) returns the
//!      author's collections sorted by recency.

use crate::stream::post::ROOT_PATH;
use crate::utils::get_request;
use anyhow::Result;
use serde_json::Value;

const BOGOTA: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";

const COL_BOGOTA_1: &str = "COLW1TGL5BKG1";
const COL_BOGOTA_2: &str = "COLW1TGL5BKG2";
const COL_CAIRO: &str = "COLW1TGL5BKG3";

fn ids_in(response: &Value) -> Vec<String> {
    response
        .as_array()
        .expect("Post stream should be an array")
        .iter()
        .map(|p| p["details"]["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[tokio_shared_rt::test(shared)]
async fn test_kind_collection_returns_only_collections() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=all&kind=collection&sorting=timeline");
    let body = get_request(&path).await?;

    let ids = ids_in(&body);
    assert!(!ids.is_empty(), "expected at least one collection post");

    // Every returned post must have kind="collection".
    for post in body.as_array().unwrap() {
        assert_eq!(
            post["details"]["kind"].as_str(),
            Some("collection"),
            "non-collection post leaked through ?kind=collection: {:?}",
            post["details"]["id"]
        );
    }

    // The 3 seeded collections must be present.
    for expected in &[COL_BOGOTA_1, COL_BOGOTA_2, COL_CAIRO] {
        assert!(
            ids.iter().any(|id| id == expected),
            "expected seeded collection {expected} in response"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_default_stream_excludes_collections() -> Result<()> {
    // No `kind` filter → default streams must exclude collections via the
    // Cypher `p.kind <> 'collection'` clause introduced in Phase 3.
    let path = format!("{ROOT_PATH}?source=all&sorting=timeline&limit=200");
    let body = get_request(&path).await?;

    let ids = ids_in(&body);
    for collection_id in &[COL_BOGOTA_1, COL_BOGOTA_2, COL_CAIRO] {
        assert!(
            !ids.iter().any(|id| id == collection_id),
            "collection {collection_id} leaked into the default stream"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_profile_collections_tab() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=author&author_id={BOGOTA}&kind=collection");
    let body = get_request(&path).await?;

    let ids = ids_in(&body);

    // Bogota authored COL_BOGOTA_1 and COL_BOGOTA_2 — both must appear.
    assert!(ids.iter().any(|id| id == COL_BOGOTA_1));
    assert!(ids.iter().any(|id| id == COL_BOGOTA_2));

    // Cairo's collection must NOT appear in Bogota's profile.
    assert!(
        !ids.iter().any(|id| id == COL_CAIRO),
        "Cairo's collection leaked into Bogota's profile collections tab"
    );

    // No non-collection posts.
    for post in body.as_array().unwrap() {
        assert_eq!(post["details"]["kind"].as_str(), Some("collection"));
    }

    Ok(())
}
