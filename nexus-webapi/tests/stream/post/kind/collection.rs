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
const EIXAMPLE: &str = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";

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
    // Baseline: no `kind` filter. Verifies (a) the default stream is
    // non-empty (so an empty-set match below is meaningful, not vacuous),
    // and (b) confirms collections are absent from the default stream.
    let baseline_path = format!("{ROOT_PATH}?source=all&sorting=timeline&limit=200");
    let baseline = get_request(&baseline_path).await?;
    let baseline_ids = ids_in(&baseline);
    assert!(
        !baseline_ids.is_empty(),
        "baseline default stream must contain at least one post"
    );
    for collection_id in &[COL_BOGOTA_1, COL_BOGOTA_2, COL_CAIRO] {
        assert!(
            !baseline_ids.iter().any(|id| id == collection_id),
            "baseline (no kind filter) must not contain collection {collection_id}"
        );
    }

    // Filtered query: the `kind=collection` filter must change the result
    // set — return collections that the baseline excluded.
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

    // The 3 seeded collections must be present in the filtered response —
    // confirming the kind filter is doing real work (returns posts the
    // baseline excluded).
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
    // Baseline assertion: with `kind=collection` set, all three seeded
    // collections ARE returned. This proves the seed data exists and the
    // suppression invariant below is meaningful (not vacuous).
    let included_path = format!("{ROOT_PATH}?source=all&kind=collection&sorting=timeline&limit=50");
    let included = get_request(&included_path).await?;
    let included_ids = ids_in(&included);
    for collection_id in &[COL_BOGOTA_1, COL_BOGOTA_2, COL_CAIRO] {
        assert!(
            included_ids.iter().any(|id| id == collection_id),
            "baseline (with kind=collection) must include {collection_id} — without this the suppression test below is vacuous"
        );
    }

    // Suppression invariant: no `kind` filter → default streams must exclude
    // collections via the Cypher `p.kind <> 'collection'` clause introduced
    // in Phase 3.
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

/// Hot stream (`?sorting=total_engagement`) is suppression invariant #4:
/// collections must never appear in the engagement-ordered timeline, even
/// once they accumulate tags / replies / reposts. The gate lives in
/// `nexus-common/src/models/post/counts.rs` where collections are excluded
/// from the engagement sorted set entirely (the read path falls back to
/// Cypher with the `p.kind <> 'collection'` clause).
#[tokio_shared_rt::test(shared)]
async fn test_hot_stream_excludes_collections() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=all&sorting=total_engagement&limit=200");
    let body = get_request(&path).await?;

    let ids = ids_in(&body);
    for collection_id in &[COL_BOGOTA_1, COL_BOGOTA_2, COL_CAIRO] {
        assert!(
            !ids.iter().any(|id| id == collection_id),
            "collection {collection_id} leaked into the hot/engagement stream"
        );
    }

    // Also: no returned post should carry kind="collection".
    for post in body.as_array().unwrap() {
        assert_ne!(
            post["details"]["kind"].as_str(),
            Some("collection"),
            "collection kind leaked into the hot/engagement stream: {:?}",
            post["details"]["id"]
        );
    }

    Ok(())
}

/// By-tag stream (`?tags=LABEL`) is suppression invariant #5: even if a
/// collection has been tagged, it must not appear in by-tag streams. The
/// gate lives in `nexus-watcher/src/events/handlers/tag.rs` where
/// `PostsByTagSearch::put_to_index` is skipped when the target post is a
/// collection (extracted helper `target_post_is_collection`).
///
/// This is a regression guard: collections in the seed data are NOT
/// currently tagged, so no collection should ever appear in a by-tag
/// stream. If a future change accidentally indexes a collection-tag
/// relationship in the by-tag sorted set, this test catches it.
#[tokio_shared_rt::test(shared)]
async fn test_by_tag_stream_excludes_collections() -> Result<()> {
    // Use a tag label that exists in seed data for non-collection posts.
    let path = format!("{ROOT_PATH}?source=all&tags=api&limit=200");
    let body = get_request(&path).await?;

    for post in body.as_array().unwrap() {
        assert_ne!(
            post["details"]["kind"].as_str(),
            Some("collection"),
            "collection leaked into the ?tags=api by-tag stream: {:?}",
            post["details"]["id"]
        );
    }

    Ok(())
}

/// `?source=bookmarks&sorting=timeline` is the Redis-path (kind-agnostic
/// bookmark sorted set). A bookmarked collection should appear regardless
/// of the global suppression invariant. Seed: Eixample bookmarks
/// `COL_BOGOTA_1`.
#[tokio_shared_rt::test(shared)]
async fn test_bookmarks_timeline_includes_bookmarked_collection() -> Result<()> {
    let path =
        format!("{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&sorting=timeline&limit=50");
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        ids.iter().any(|id| id == COL_BOGOTA_1),
        "Eixample's bookmarked collection must appear in the timeline bookmarks stream"
    );
    Ok(())
}

/// `?source=bookmarks&sorting=total_engagement` is the Cypher-fallback path
/// (`can_use_index` returns false for bookmarks + engagement sort). Without
/// the new `source=bookmarks` exemption in the kind-filter match, the
/// default `p.kind <> 'collection'` clause silently dropped bookmarked
/// collections, contradicting the public OpenAPI promise that bookmarks
/// are exempt. This is the load-bearing test for the Cypher exemption.
#[tokio_shared_rt::test(shared)]
async fn test_bookmarks_engagement_sort_includes_bookmarked_collection() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&sorting=total_engagement&limit=50"
    );
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        ids.iter().any(|id| id == COL_BOGOTA_1),
        "Eixample's bookmarked collection must appear in the engagement-sorted bookmarks stream (Cypher fallback exemption)"
    );
    Ok(())
}
