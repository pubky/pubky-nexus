use crate::stream::post::utils::verify_post_list_kind;
use crate::stream::post::BOGOTA;
use crate::stream::post::ROOT_PATH;
use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::Value;

const KIND: &str = "collection";

const EIXAMPLE: &str = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";

const COL_BOGOTA_1: &str = "COLW1TGL5BKG1";
const COL_BOGOTA_2: &str = "COLW1TGL5BKG2";
const COL_CAIRO: &str = "COLW1TGL5BKG3";
// Debug-fixture Collections seeded for `?source=collection` tests below.
// They participate in the global Collection set just like any other.
const COL_BOGOTA_MALF: &str = "MALF1TGL5BKG7";
const COL_BOGOTA_NEST: &str = "NEST1TGL5BKG8";
const SHORT_BOGOTA: &str = "A5D6P9V3Q0T";

const ALL_COLLECTIONS: &[&str] = &[
    COL_BOGOTA_1,
    COL_BOGOTA_2,
    COL_CAIRO,
    COL_BOGOTA_MALF,
    COL_BOGOTA_NEST,
];

fn ids_in(response: &Value) -> Vec<String> {
    response
        .as_array()
        .expect("Post stream should be an array")
        .iter()
        .map(|p| p["details"]["id"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn contains_any_collection(ids: &[String]) -> bool {
    ids.iter().any(|id| ALL_COLLECTIONS.contains(&id.as_str()))
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_collection_post_kind() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=collection");

    let body = get_request(&path).await?;
    let post_list = vec![
        COL_BOGOTA_NEST,
        COL_BOGOTA_MALF,
        COL_CAIRO,
        COL_BOGOTA_2,
        COL_BOGOTA_1,
    ];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_collection_post_kind_with_author() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=collection&author_id={BOGOTA}&source=author");

    let body = get_request(&path).await?;
    let post_list = vec![COL_BOGOTA_NEST, COL_BOGOTA_MALF, COL_BOGOTA_2, COL_BOGOTA_1];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_default_stream_includes_collections() -> Result<()> {
    // Asserts Collections reach POST_TIMELINE_KEY_PARTS via the Redis path.
    let path = format!("{ROOT_PATH}?source=all&sorting=timeline&limit=30");
    let body = get_request(&path).await?;
    assert!(
        contains_any_collection(&ids_in(&body)),
        "Default timeline must include at least one Collection"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_stream_includes_collections() -> Result<()> {
    // Asserts Collections reach POST_TOTAL_ENGAGEMENT. Seed has 30+ posts at
    // engagement ≥ 1 and Collections sit at 0–1, so the DESC top-30 doesn't
    // reach them — query ASC to walk the set from the bottom.
    let path = format!("{ROOT_PATH}?source=all&sorting=total_engagement&order=ascending&limit=30");
    let body = get_request(&path).await?;
    assert!(
        contains_any_collection(&ids_in(&body)),
        "Engagement sorted set must include at least one Collection"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_by_tag_stream_includes_tagged_collection() -> Result<()> {
    // Redis by-tag path (PostsByTagSearch). Seed tags COL_CAIRO with `api`
    // (see docker/test-graph/mocks/posts.cypher near the Collection block).
    let path = format!("{ROOT_PATH}?source=all&tags=api&limit=30");
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        ids.iter().any(|id| id == COL_CAIRO),
        "By-tag stream for `api` must include the tagged Collection {COL_CAIRO}"
    );
    Ok(())
}

/// `?source=bookmarks&kind=collection` intersects the user's bookmark sorted
/// set with `p.kind = 'collection'`, returning only their bookmarked
/// collections.
///
/// Seed: Eixample bookmarks both `COL_BOGOTA_1` (a Collection) and
/// `SHORT_BOGOTA` (a `kind=short` post). The kind filter must include the
/// former and exclude the latter — proving the intersection is doing real
/// work, not just falling back to the kind-agnostic bookmarks stream.
#[tokio_shared_rt::test(shared)]
async fn test_bookmarks_with_kind_collection_returns_only_bookmarked_collections() -> Result<()> {
    // Baseline: without the kind filter, Eixample's bookmarks stream contains
    // both bookmarks. Locks in the seed and proves the kind filter below is
    // non-vacuous.
    let baseline_path =
        format!("{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&sorting=timeline&limit=50");
    let baseline = get_request(&baseline_path).await?;
    let baseline_ids = ids_in(&baseline);
    assert!(
        baseline_ids.iter().any(|id| id == COL_BOGOTA_1),
        "baseline: Eixample bookmarks must include {COL_BOGOTA_1}"
    );
    assert!(
        baseline_ids.iter().any(|id| id == SHORT_BOGOTA),
        "baseline: Eixample bookmarks must include {SHORT_BOGOTA} (proves the kind filter is non-vacuous)"
    );

    let path = format!(
        "{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&kind=collection&sorting=timeline&limit=50"
    );
    let body = get_request(&path).await?;
    let ids = ids_in(&body);

    assert!(
        ids.iter().any(|id| id == COL_BOGOTA_1),
        "kind=collection filter must include the bookmarked Collection {COL_BOGOTA_1}"
    );
    assert!(
        !ids.iter().any(|id| id == SHORT_BOGOTA),
        "kind=collection filter must exclude the non-collection bookmark {SHORT_BOGOTA}"
    );
    for post in body.as_array().unwrap() {
        assert_eq!(
            post["details"]["kind"].as_str(),
            Some("collection"),
            "non-collection post leaked through ?source=bookmarks&kind=collection: {:?}",
            post["details"]["id"]
        );
    }

    Ok(())
}

/// Same `?source=bookmarks&kind=collection` intersection but engagement sort
/// — hits a different ORDER BY branch in the Cypher fallback. Both sorts
/// route through Cypher (any kind filter short-circuits `can_use_index`),
/// but the engagement branch joins the engagement score subquery so it's
/// worth covering independently.
#[tokio_shared_rt::test(shared)]
async fn test_bookmarks_with_kind_collection_engagement_sort_returns_only_collections() -> Result<()>
{
    // Baseline: without the kind filter, the engagement-sorted bookmarks
    // stream contains both bookmarks. Proves the kind filter below is
    // non-vacuous — the exclusion assertion would otherwise pass trivially
    // if SHORT_BOGOTA never appeared in the unfiltered result.
    let baseline_path = format!(
        "{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&sorting=total_engagement&limit=50"
    );
    let baseline = get_request(&baseline_path).await?;
    let baseline_ids = ids_in(&baseline);
    assert!(
        baseline_ids.iter().any(|id| id == COL_BOGOTA_1),
        "baseline: engagement-sorted bookmarks must include {COL_BOGOTA_1}"
    );
    assert!(
        baseline_ids.iter().any(|id| id == SHORT_BOGOTA),
        "baseline: engagement-sorted bookmarks must include {SHORT_BOGOTA} (proves the kind filter is non-vacuous)"
    );

    let path = format!(
        "{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&kind=collection&sorting=total_engagement&limit=50"
    );
    let body = get_request(&path).await?;
    let ids = ids_in(&body);

    assert!(
        ids.iter().any(|id| id == COL_BOGOTA_1),
        "engagement-sort: kind=collection must include the bookmarked Collection {COL_BOGOTA_1}"
    );
    assert!(
        !ids.iter().any(|id| id == SHORT_BOGOTA),
        "engagement-sort: kind=collection must exclude the non-collection bookmark {SHORT_BOGOTA}"
    );
    for post in body.as_array().unwrap() {
        assert_eq!(
            post["details"]["kind"].as_str(),
            Some("collection"),
            "non-collection post leaked through ?source=bookmarks&kind=collection&sorting=total_engagement: {:?}",
            post["details"]["id"]
        );
    }

    Ok(())
}

// ?source=collection seed: COLW1TGL5BKG1 has 5 items — 3 live posts (A, C, K),
// 1 missing-post URI, 1 non-post URI (profile.json). Expected stream: [A, C, K].

const POST_A: &str = "A5D6P9V3Q0T";
const POST_C: &str = "C3L7W0F9Q4K8";
const POST_K: &str = "K1P6Q9M2X4J8";
const MISSING_POST_ID: &str = "ZZZZZZZZZZZZZ";

#[tokio_shared_rt::test(shared)]
async fn test_source_collection_returns_items_in_curator_order() -> Result<()> {
    let path =
        format!("{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&limit=50");
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert_eq!(
        ids,
        vec![POST_A.to_string(), POST_C.to_string(), POST_K.to_string()],
        "source=collection must return live items in curator order, with missing/non-post URIs dropped"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_source_collection_paginates_with_skip_limit() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&skip=1&limit=2"
    );
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert_eq!(
        ids,
        vec![POST_C.to_string(), POST_K.to_string()],
        "skip=1&limit=2 must return the middle slice"
    );
    Ok(())
}

/// Load-bearing: missing posts and non-post URIs are both dropped.
/// FE diffs against the envelope's items[] to recover them.
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_drops_missing_and_non_post_items() -> Result<()> {
    let path =
        format!("{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&limit=50");
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        !ids.iter().any(|id| id == MISSING_POST_ID),
        "missing post {MISSING_POST_ID} must be dropped (no PostDetails in index)"
    );
    // Non-post URIs (profile.json) drop at the ParsedUri match.
    assert_eq!(
        ids.len(),
        3,
        "expected exactly 3 live items (5 - 1 missing - 1 non-post)"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_source_collection_for_non_collection_post_returns_empty() -> Result<()> {
    // POST_A is a kind=short post; pointing source=collection at it returns empty.
    let path = format!("{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={POST_A}");
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        ids.is_empty(),
        "source=collection on a non-Collection post must return empty, got: {ids:?}"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_source_collection_for_unknown_post_returns_empty() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id=NONEXISTENT99");
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        ids.is_empty(),
        "source=collection on an unknown post must return empty, got: {ids:?}"
    );
    Ok(())
}

/// Empty `items` → empty stream (no error). Seed: COL_BOGOTA_2.
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_for_empty_collection_returns_empty() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_2}");
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        ids.is_empty(),
        "empty Collection must return empty stream, got: {ids:?}"
    );
    Ok(())
}

/// Skip past end → empty.
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_skip_past_end_returns_empty() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&skip=100&limit=10"
    );
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        ids.is_empty(),
        "skip=100 on a 5-item Collection must return empty, got: {ids:?}"
    );
    Ok(())
}

/// `tags` rejected with 400 (curator order, no tag filtering).
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_with_tags_rejected_400() -> Result<()> {
    let path =
        format!("{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&tags=foo");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

/// `kind` rejected with 400 (source already determines the set).
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_with_kind_rejected_400() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&kind=short"
    );
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

/// `sorting` rejected with 400 (curator order is intrinsic).
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_with_sorting_rejected_400() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&sorting=timeline"
    );
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

/// `order` rejected with 400 (curator order is intrinsic).
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_with_order_rejected_400() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&order=descending"
    );
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

/// `start` rejected with 400 (no score axis; use skip/limit).
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_with_start_rejected_400() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&start=100"
    );
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

/// `end` rejected with 400 (same as `start`).
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_with_end_rejected_400() -> Result<()> {
    let path =
        format!("{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&end=200");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

/// Malformed envelope JSON → empty stream (no 500). Seed: MALF1TGL5BKG7.
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_with_malformed_envelope_returns_empty() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id=MALF1TGL5BKG7");
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        ids.is_empty(),
        "malformed Collection envelope must return empty stream (defensive parse), got: {ids:?}"
    );
    Ok(())
}

/// viewer_id hydrates per-viewer fields. Seed: Eixample bookmarks POST_A.
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_honors_viewer_id_for_bookmark_hydration() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&viewer_id={EIXAMPLE}"
    );
    let body = get_request(&path).await?;
    let items = body.as_array().expect("response must be an array");
    let item_a = items
        .iter()
        .find(|v| v["details"]["id"].as_str() == Some(POST_A))
        .expect("POST_A must appear in the collection items response");
    assert!(
        item_a["bookmark"].is_object(),
        "viewer_id must populate the bookmark field on POST_A, got: {item_a}"
    );
    Ok(())
}

/// Nested Collections surface as normal items (Collections ARE posts).
/// BE does not recurse — one level only. Seed: NEST1TGL5BKG8 → COLW1TGL5BKG1.
#[tokio_shared_rt::test(shared)]
async fn test_source_collection_accepts_nested_collection_post_uris() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id=NEST1TGL5BKG8");
    let body = get_request(&path).await?;
    let items = body.as_array().expect("response must be an array");
    assert_eq!(
        items.len(),
        1,
        "expected exactly 1 item (the nested Collection)"
    );
    assert_eq!(items[0]["details"]["id"].as_str(), Some(COL_BOGOTA_1));
    assert_eq!(items[0]["details"]["kind"].as_str(), Some("collection"));
    Ok(())
}
