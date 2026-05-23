use crate::stream::post::utils::verify_post_list_kind;
use crate::stream::post::BOGOTA;
use crate::stream::post::ROOT_PATH;
use crate::utils::get_request;
use anyhow::Result;
use serde_json::Value;

const KIND: &str = "collection";

const EIXAMPLE: &str = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";

const COL_BOGOTA_1: &str = "COLW1TGL5BKG1";
const COL_BOGOTA_2: &str = "COLW1TGL5BKG2";
const COL_CAIRO: &str = "COLW1TGL5BKG3";
const SHORT_BOGOTA: &str = "A5D6P9V3Q0T";

const ALL_COLLECTIONS: &[&str] = &[COL_BOGOTA_1, COL_BOGOTA_2, COL_CAIRO];

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
    let post_list = vec![COL_CAIRO, COL_BOGOTA_2, COL_BOGOTA_1];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_collection_post_kind_with_author() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=collection&author_id={BOGOTA}&source=author");

    let body = get_request(&path).await?;
    let post_list = vec![COL_BOGOTA_2, COL_BOGOTA_1];
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
