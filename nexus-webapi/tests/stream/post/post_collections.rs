//! `?source=post_collections`: the Collection posts that curate a given post,
//! served from the COLLECTED edges. Seeds (posts.cypher): COLW1TGL5BKG1 and the
//! newer COLW1TGL5BKG2 curate SHORT_BOGOTA, NEST1TGL5BKG8 curates COLW1TGL5BKG1,
//! Eixample bookmarks COLW1TGL5BKG1.

use super::kind::{COL_BOGOTA_1, COL_BOGOTA_2, COL_BOGOTA_NEST, COL_CAIRO, EIXAMPLE, SHORT_BOGOTA};
use super::utils::ids_in;
use super::{BOGOTA, KEYS_ROOT_PATH, ROOT_PATH};
use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;

fn path(post_id: &str, extra: &str) -> String {
    format!("{ROOT_PATH}?source=post_collections&author_id={BOGOTA}&post_id={post_id}{extra}")
}

#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_returns_curators_newest_first() -> Result<()> {
    let body = get_request(&path(SHORT_BOGOTA, "")).await?;
    assert_eq!(
        ids_in(&body),
        vec![COL_BOGOTA_2.to_string(), COL_BOGOTA_1.to_string()]
    );
    assert_eq!(body[0]["details"]["kind"].as_str(), Some("collection"));
    assert_eq!(body[0]["details"]["author"].as_str(), Some(BOGOTA));
    Ok(())
}

/// A Collection is a post, so it can be curated by another Collection.
#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_for_nested_collection() -> Result<()> {
    let body = get_request(&path(COL_BOGOTA_1, "")).await?;
    assert_eq!(ids_in(&body), vec![COL_BOGOTA_NEST.to_string()]);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_for_uncurated_post_returns_empty() -> Result<()> {
    let body = get_request(&path(COL_CAIRO, "")).await?;
    let ids = ids_in(&body);
    assert!(
        ids.is_empty(),
        "no collection curates COL_CAIRO, got: {ids:?}"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_for_unknown_post_returns_empty() -> Result<()> {
    let body = get_request(&path("NONEXISTENT99", "")).await?;
    let ids = ids_in(&body);
    assert!(
        ids.is_empty(),
        "unknown post must return empty, got: {ids:?}"
    );
    Ok(())
}

/// Post ids are globally unique, so the author must still be the real one.
#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_with_wrong_author_returns_empty() -> Result<()> {
    let path =
        format!("{ROOT_PATH}?source=post_collections&author_id={EIXAMPLE}&post_id={SHORT_BOGOTA}");
    let body = get_request(&path).await?;
    let ids = ids_in(&body);
    assert!(
        ids.is_empty(),
        "wrong author must return empty, got: {ids:?}"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_paginates_with_skip_limit() -> Result<()> {
    let body = get_request(&path(SHORT_BOGOTA, "&limit=1")).await?;
    assert_eq!(ids_in(&body), vec![COL_BOGOTA_2.to_string()]);
    let body = get_request(&path(SHORT_BOGOTA, "&skip=1&limit=1")).await?;
    assert_eq!(ids_in(&body), vec![COL_BOGOTA_1.to_string()]);
    let body = get_request(&path(SHORT_BOGOTA, "&skip=2")).await?;
    let ids = ids_in(&body);
    assert!(ids.is_empty(), "skip past both curators, got: {ids:?}");
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_rejects_kind_filters_400() -> Result<()> {
    invalid_get_request(&path(SHORT_BOGOTA, "&kind=short"), StatusCode::BAD_REQUEST).await?;
    invalid_get_request(
        &path(SHORT_BOGOTA, "&exclude_kinds=collection"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_requires_both_ids_400() -> Result<()> {
    let missing_author = format!("{ROOT_PATH}?source=post_collections&post_id={SHORT_BOGOTA}");
    invalid_get_request(&missing_author, StatusCode::BAD_REQUEST).await?;
    let missing_post = format!("{ROOT_PATH}?source=post_collections&author_id={BOGOTA}");
    invalid_get_request(&missing_post, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

/// viewer_id hydrates per-viewer fields on the returned collections.
#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_honors_viewer_id() -> Result<()> {
    let body = get_request(&path(SHORT_BOGOTA, &format!("&viewer_id={EIXAMPLE}"))).await?;
    let bookmarked = body
        .as_array()
        .expect("array")
        .iter()
        .find(|v| v["details"]["id"].as_str() == Some(COL_BOGOTA_1))
        .expect("COL_BOGOTA_1 present");
    assert!(
        bookmarked["bookmark"].is_object(),
        "Eixample bookmarked COL_BOGOTA_1, got: {bookmarked}"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_source_post_collections_keys() -> Result<()> {
    let path = format!(
        "{KEYS_ROOT_PATH}?source=post_collections&author_id={BOGOTA}&post_id={SHORT_BOGOTA}"
    );
    let body = get_request(&path).await?;
    let keys: Vec<&str> = body["post_keys"]
        .as_array()
        .expect("post_keys array")
        .iter()
        .filter_map(|k| k.as_str())
        .collect();
    assert_eq!(
        keys,
        vec![
            format!("{BOGOTA}:{COL_BOGOTA_2}").as_str(),
            format!("{BOGOTA}:{COL_BOGOTA_1}").as_str()
        ]
    );
    Ok(())
}
