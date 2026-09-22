//! `exclude_kinds` filter: server-side kind exclusion on post streams.
//!
//! Motivation: the FE "All" feed used to filter collections client-side,
//! which yields empty pages when a run of collections sits at the head of
//! the timeline. `exclude_kinds` filters in the Cypher WHERE clause, before
//! ORDER/SKIP/LIMIT, so pages always come back full.

use super::link::POST_L1;
use super::{ALL_COLLECTIONS, COL_BOGOTA_1, COL_CAIRO, EIXAMPLE, SHORT_BOGOTA};
use crate::stream::post::utils::ids_in;
use crate::stream::post::{BOGOTA, KEYS_ROOT_PATH, ROOT_PATH};
use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::Value;

fn assert_no_kind(response: &Value, excluded: &[&str]) {
    for post in response.as_array().expect("Post stream should be an array") {
        let kind = post["details"]["kind"].as_str().unwrap_or_default();
        assert!(
            !excluded.contains(&kind),
            "excluded kind `{kind}` leaked through exclude_kinds: {:?}",
            post["details"]["id"]
        );
    }
}

/// The headline assertion: the seed's two globally-newest posts are
/// collections, so the unfiltered head of the timeline is a collection run.
/// With `exclude_kinds=collection` the first page must still come back full
/// (the empty-page failure the FE hit with client-side filtering).
#[tokio_shared_rt::test(shared)]
async fn test_exclude_collections_timeline_returns_full_page() -> Result<()> {
    // Baseline: without the filter the first page contains collections,
    // proving the exclusion below is non-vacuous.
    let baseline =
        get_request(&format!("{ROOT_PATH}?source=all&sorting=timeline&limit=10")).await?;
    assert!(
        ids_in(&baseline)
            .iter()
            .any(|id| ALL_COLLECTIONS.contains(&id.as_str())),
        "baseline: unfiltered timeline head must contain a Collection"
    );

    let body = get_request(&format!(
        "{ROOT_PATH}?source=all&sorting=timeline&exclude_kinds=collection&limit=10"
    ))
    .await?;
    let ids = ids_in(&body);
    assert_eq!(ids.len(), 10, "excluded page must still be full");
    assert!(
        !ids.iter().any(|id| ALL_COLLECTIONS.contains(&id.as_str())),
        "no Collection id may appear with exclude_kinds=collection"
    );
    assert_no_kind(&body, &["collection"]);
    Ok(())
}

/// Engagement-sort exclusion on the graph path, with a graph-path baseline:
/// `source=bookmarks&sorting=total_engagement` routes to Cypher even without
/// a kind filter (only timeline-sorted bookmarks have a sorted set), so both
/// requests exercise the same query shape and the baseline proves the
/// exclusion below is non-vacuous.
#[tokio_shared_rt::test(shared)]
async fn test_exclude_collections_engagement_sort() -> Result<()> {
    let base =
        format!("{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&sorting=total_engagement");
    let baseline_ids = ids_in(&get_request(&format!("{base}&limit=50")).await?);
    assert!(
        baseline_ids.iter().any(|id| id == COL_BOGOTA_1),
        "baseline: engagement-sorted bookmarks must include {COL_BOGOTA_1}"
    );

    let body = get_request(&format!("{base}&exclude_kinds=collection&limit=50")).await?;
    let ids = ids_in(&body);
    assert!(
        ids.iter().any(|id| id == SHORT_BOGOTA),
        "bookmarked non-collection post must remain"
    );
    assert!(
        !ids.iter().any(|id| id == COL_BOGOTA_1),
        "bookmarked Collection must be excluded"
    );
    assert_no_kind(&body, &["collection"]);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_exclude_multiple_kinds() -> Result<()> {
    let body = get_request(&format!(
        "{ROOT_PATH}?source=all&exclude_kinds=collection,link&limit=50"
    ))
    .await?;
    let ids = ids_in(&body);
    assert!(!ids.is_empty());
    // Spot-check the newest seeded link post on top of the kind sweep.
    assert!(
        !ids.iter().any(|id| id == POST_L1),
        "link post leaked through exclude_kinds=collection,link"
    );
    assert_no_kind(&body, &["collection", "link"]);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_exclude_kinds_author_source() -> Result<()> {
    let body = get_request(&format!(
        "{ROOT_PATH}?source=author&author_id={BOGOTA}&exclude_kinds=collection&limit=50"
    ))
    .await?;
    assert!(
        ids_in(&body).iter().any(|id| id == SHORT_BOGOTA),
        "author stream must keep Bogota's non-collection posts"
    );
    assert_no_kind(&body, &["collection"]);
    Ok(())
}

/// Inverse of the `?source=bookmarks&kind=collection` intersection tests in
/// collection.rs (which also lock in the seed: Eixample bookmarks both
/// `COL_BOGOTA_1` and `SHORT_BOGOTA`).
#[tokio_shared_rt::test(shared)]
async fn test_exclude_kinds_bookmarks_source() -> Result<()> {
    let body = get_request(&format!(
        "{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&exclude_kinds=collection&sorting=timeline&limit=50"
    ))
    .await?;
    let ids = ids_in(&body);
    assert!(
        ids.iter().any(|id| id == SHORT_BOGOTA),
        "bookmarked non-collection post must remain"
    );
    assert!(
        !ids.iter().any(|id| id == COL_BOGOTA_1),
        "bookmarked Collection must be excluded"
    );
    assert_no_kind(&body, &["collection"]);
    Ok(())
}

/// Bookmarked replies must survive kind filtering. The Redis bookmarks stream
/// returns every bookmark (replies included), but any kind filter routes
/// bookmarks to the graph, whose parents-only filter would otherwise drop
/// bookmarked replies. Seed: Eixample bookmarks the short reply
/// `0000003A0BJWT`. It must appear both unfiltered and under
/// `exclude_kinds=collection`.
#[tokio_shared_rt::test(shared)]
async fn test_exclude_kinds_keeps_bookmarked_replies() -> Result<()> {
    const BOOKMARKED_REPLY: &str = "0000003A0BJWT";
    let base = format!("{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&sorting=timeline");

    let baseline_ids = ids_in(&get_request(&format!("{base}&limit=50")).await?);
    assert!(
        baseline_ids.iter().any(|id| id == BOOKMARKED_REPLY),
        "baseline: bookmarks stream must include the bookmarked reply"
    );

    let ids = ids_in(&get_request(&format!("{base}&exclude_kinds=collection&limit=50")).await?);
    assert!(
        ids.iter().any(|id| id == BOOKMARKED_REPLY),
        "bookmarked reply must survive exclude_kinds on the graph path"
    );
    assert!(
        !ids.iter().any(|id| id == COL_BOGOTA_1),
        "bookmarked Collection must still be excluded"
    );
    Ok(())
}

/// Tags + exclusion combine on the Cypher path. Seed tags `COL_CAIRO` with
/// `api` (baseline in `test_by_tag_stream_includes_tagged_collection`).
#[tokio_shared_rt::test(shared)]
async fn test_exclude_kinds_with_tags() -> Result<()> {
    let body = get_request(&format!(
        "{ROOT_PATH}?source=all&tags=api&exclude_kinds=collection&limit=30"
    ))
    .await?;
    assert!(
        !ids_in(&body).iter().any(|id| id == COL_CAIRO),
        "tagged Collection must be excluded from the by-tag stream"
    );
    assert_no_kind(&body, &["collection"]);
    Ok(())
}

/// WHERE precedes SKIP/LIMIT, so offset pagination over an excluded stream is
/// contiguous: two half pages equal one full page.
#[tokio_shared_rt::test(shared)]
async fn test_exclude_kinds_pagination_contiguous() -> Result<()> {
    let base = format!("{ROOT_PATH}?source=all&sorting=timeline&exclude_kinds=collection");
    let page_1 = ids_in(&get_request(&format!("{base}&limit=5")).await?);
    let page_2 = ids_in(&get_request(&format!("{base}&skip=5&limit=5")).await?);
    let full = ids_in(&get_request(&format!("{base}&limit=10")).await?);

    assert_eq!(page_1.len(), 5);
    let combined: Vec<String> = page_1.into_iter().chain(page_2).collect();
    assert_eq!(combined, full, "paged fetches must equal the single fetch");
    Ok(())
}

/// The keys endpoint honors the filter and still returns the cursor.
#[tokio_shared_rt::test(shared)]
async fn test_exclude_kinds_post_keys_with_cursor() -> Result<()> {
    let body = get_request(&format!(
        "{KEYS_ROOT_PATH}?source=all&sorting=timeline&exclude_kinds=collection&limit=5"
    ))
    .await?;
    let keys: Vec<&str> = body["post_keys"]
        .as_array()
        .expect("post_keys should be an array")
        .iter()
        .map(|k| k.as_str().unwrap_or_default())
        .collect();
    assert_eq!(keys.len(), 5);
    for key in &keys {
        let post_id = key.split(':').nth(1).unwrap_or_default();
        assert!(
            !ALL_COLLECTIONS.contains(&post_id),
            "Collection key {key} leaked through exclude_kinds=collection"
        );
    }
    let cursor = body["last_post_score"]
        .as_u64()
        .expect("last_post_score cursor must be present");

    // Resuming from the cursor keeps excluding.
    let next = get_request(&format!(
        "{KEYS_ROOT_PATH}?source=all&sorting=timeline&exclude_kinds=collection&limit=5&start={cursor}"
    ))
    .await?;
    for key in next["post_keys"].as_array().expect("post_keys array") {
        let key = key.as_str().unwrap_or_default();
        let post_id = key.split(':').nth(1).unwrap_or_default();
        assert!(
            !ALL_COLLECTIONS.contains(&post_id),
            "Collection key {key} leaked after cursor resume"
        );
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_exclude_kinds_rejects_invalid_values() -> Result<()> {
    for path in [
        // Unrecognized kind: strict parsing, unlike the lenient `kind` param.
        format!("{ROOT_PATH}?exclude_kinds=bogus"),
        // `unknown` is the serde catch-all, not an excludable kind.
        format!("{ROOT_PATH}?exclude_kinds=unknown"),
        // Mutually exclusive with `kind`.
        format!("{ROOT_PATH}?kind=short&exclude_kinds=collection"),
        // Incompatible with the curator-ordered collection source.
        format!(
            "{ROOT_PATH}?source=collection&author_id={BOGOTA}&post_id={COL_BOGOTA_1}&exclude_kinds=collection"
        ),
        // Reply sources: the Cypher fallback has no reply MATCH arm, so kind
        // filters are rejected instead of silently returning parent posts.
        format!(
            "{ROOT_PATH}?source=post_replies&author_id={BOGOTA}&post_id={SHORT_BOGOTA}&exclude_kinds=collection"
        ),
        format!("{ROOT_PATH}?source=author_replies&author_id={BOGOTA}&exclude_kinds=collection"),
        format!(
            "{ROOT_PATH}?source=post_replies&author_id={BOGOTA}&post_id={SHORT_BOGOTA}&kind=short"
        ),
        format!("{ROOT_PATH}?source=author_replies&author_id={BOGOTA}&kind=short"),
        // Empty list.
        format!("{ROOT_PATH}?exclude_kinds="),
        // Over the 7-item bound.
        format!("{ROOT_PATH}?exclude_kinds=short,long,image,video,link,file,collection,short"),
    ] {
        invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    }
    Ok(())
}

/// Excluding every known kind is allowed; the seed has no NULL/unknown-kind
/// posts, so the stream is simply empty (a 200, not an error).
#[tokio_shared_rt::test(shared)]
async fn test_exclude_all_kinds_returns_empty() -> Result<()> {
    let body = get_request(&format!(
        "{ROOT_PATH}?source=all&exclude_kinds=short,long,image,video,link,file,collection&limit=10"
    ))
    .await?;
    assert!(ids_in(&body).is_empty());
    Ok(())
}
