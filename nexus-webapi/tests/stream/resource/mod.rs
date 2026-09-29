use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::Value;

// The resource stream is served from the graph. These tests pin the seeded
// resources from docker/test-graph/mocks/resources.cypher:
//
// - ARTICLE: bitcoin by amsterdam (mapky, ..095000) and bogota (mapky, ..095001),
//            interesting by amsterdam (eventky, ..095002)
// - EVENT:   calendar by bogota (eventky, ..095003)
// - VIDEO:   bitcoin by amsterdam (mapky, ..095004)
//
// Watcher tests share the graph and may add resources of their own, so the
// assertions are about the seeded IDs: their presence, relative order and
// scores, never the exact page.

const ROOT_PATH: &str = "/v0/stream/resources";
const IDS_PATH: &str = "/v0/stream/resources/ids";

const ARTICLE: &str = "450a72e3da164bfc3ac5f4056f9e5c7c";
const EVENT: &str = "fb4155a2295ff3a8a8fe02e28229c021";
const VIDEO: &str = "e23f778c4f2a84606f350e4df1a918e9";

const ARTICLE_LATEST_TAG: u64 = 1724544095002;
const EVENT_LATEST_TAG: u64 = 1724544095003;

async fn get_ids(query: &str) -> Result<(Vec<String>, Option<u64>)> {
    let body = get_request(&format!("{IDS_PATH}?{query}")).await?;
    let ids = body["resource_ids"]
        .as_array()
        .expect("resource_ids should be an array")
        .iter()
        .map(|v| v.as_str().expect("resource id").to_string())
        .collect();
    Ok((ids, body["last_score"].as_u64()))
}

async fn get_views(query: &str) -> Result<Vec<Value>> {
    let body = get_request(&format!("{ROOT_PATH}?{query}")).await?;
    Ok(body
        .as_array()
        .expect("Should return array of ResourceView")
        .clone())
}

fn position(ids: &[String], id: &str) -> usize {
    ids.iter()
        .position(|x| x == id)
        .unwrap_or_else(|| panic!("{id} should be in the stream: {ids:?}"))
}

fn assert_before(ids: &[String], first: &str, second: &str) {
    assert!(
        position(ids, first) < position(ids, second),
        "{first} should come before {second}: {ids:?}"
    );
}

// =============================================
// GET /v0/stream/resources (returns Vec<ResourceView>)
// =============================================

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_all() -> Result<()> {
    let views = get_views("sorting=timeline&limit=100").await?;
    assert!(!views.is_empty(), "the seeded resources should stream");

    for view in &views {
        assert!(view["details"].is_object(), "Should have details");
        assert!(view["details"]["id"].is_string(), "Should have id");
        assert!(view["details"]["uri"].is_string(), "Should have uri");
        assert!(view["details"]["scheme"].is_string(), "Should have scheme");
        assert!(view["tags"].is_array(), "Should have tags array");
        assert!(
            view["taggers_count"].is_number(),
            "Should have taggers_count"
        );
    }

    let ids: Vec<String> = views
        .iter()
        .map(|v| v["details"]["id"].as_str().unwrap().to_string())
        .collect();
    // Relative order under timeline; the latest-tag-vs-node-indexed_at
    // distinction is pinned by the cursor-bound test below, whose
    // `end={EVENT_LATEST_TAG}` only holds under MAX(t.indexed_at)
    assert_before(&ids, VIDEO, EVENT);
    assert_before(&ids, EVENT, ARTICLE);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_by_app_mapky() -> Result<()> {
    let (ids, _) = get_ids("app=mapky&sorting=timeline&limit=100").await?;
    assert_before(&ids, VIDEO, ARTICLE);
    assert!(
        !ids.contains(&EVENT.to_string()),
        "the event has no mapky tag"
    );

    let views = get_views("app=mapky&sorting=timeline&limit=100").await?;
    assert!(
        !views.is_empty(),
        "Mapky app filter should return resources"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_by_app_eventky() -> Result<()> {
    // The article's "interesting" tag is from eventky, so it is in this stream too
    let (ids, _) = get_ids("app=eventky&sorting=timeline&limit=100").await?;
    assert_before(&ids, EVENT, ARTICLE);
    assert!(
        !ids.contains(&VIDEO.to_string()),
        "the video has no eventky tag"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_by_tag_bitcoin() -> Result<()> {
    let (ids, _) = get_ids("tags=bitcoin&sorting=timeline&limit=100").await?;
    // Within the label, the video's bitcoin tag is the newest
    assert_before(&ids, VIDEO, ARTICLE);
    assert!(
        !ids.contains(&EVENT.to_string()),
        "the event is not tagged bitcoin"
    );

    let views = get_views("tags=bitcoin&sorting=timeline&limit=100").await?;
    assert!(
        !views.is_empty(),
        "Bitcoin tag filter should return resources"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_multi_tag_or() -> Result<()> {
    let (ids, _) = get_ids("tags=bitcoin,calendar&sorting=timeline&limit=100").await?;
    for id in [ARTICLE, EVENT, VIDEO] {
        position(&ids, id);
    }
    assert_before(&ids, VIDEO, EVENT);
    assert_before(&ids, EVENT, ARTICLE);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_combined_app_and_tag() -> Result<()> {
    let (ids, _) = get_ids("app=mapky&tags=bitcoin&sorting=timeline&limit=100").await?;
    assert_before(&ids, VIDEO, ARTICLE);
    assert!(!ids.contains(&EVENT.to_string()));

    // The article's eventky tag is "interesting", not bitcoin
    let (ids, _) = get_ids("app=eventky&tags=bitcoin&sorting=timeline&limit=100").await?;
    assert!(
        !ids.contains(&ARTICLE.to_string()),
        "no bitcoin tag from eventky on the article"
    );

    let views = get_views("app=mapky&tags=bitcoin&sorting=timeline&limit=100").await?;
    assert!(
        !views.is_empty(),
        "Mapky+bitcoin combined filter should return resources"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_rejects_taggers_count_sorting() -> Result<()> {
    for path in [ROOT_PATH, IDS_PATH] {
        invalid_get_request(
            &format!("{path}?sorting=taggers_count"),
            StatusCode::BAD_REQUEST,
        )
        .await?;
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_pagination() -> Result<()> {
    let views = get_views("sorting=timeline&limit=1").await?;
    assert_eq!(views.len(), 1, "Should respect limit=1");

    Ok(())
}

// =============================================
// GET /v0/stream/resources/ids (returns ResourceKeyStream)
// =============================================

#[tokio_shared_rt::test(shared)]
async fn test_stream_resource_ids() -> Result<()> {
    let (ids, last_score) = get_ids("sorting=timeline").await?;
    assert!(!ids.is_empty(), "Should return resources");
    assert!(last_score.is_some(), "a non-empty page carries a cursor");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resource_ids_cursor_pagination() -> Result<()> {
    // `eventky` restricted to the two seeded labels holds exactly the seeded
    // event ("calendar") and article ("interesting"), so the pages below are
    // exact by construction: nothing else in the shared graph can join them.
    let filter = "app=eventky&tags=calendar,interesting&sorting=timeline";

    let (page1, cursor) = get_ids(&format!("{filter}&limit=1")).await?;
    assert_eq!(page1, vec![EVENT.to_string()]);
    assert_eq!(cursor, Some(EVENT_LATEST_TAG));

    // Resume at the cursor: it is inclusive, so skip the row it repeats
    let cursor = cursor.unwrap();
    let (page2, cursor2) = get_ids(&format!("{filter}&limit=1&start={cursor}&skip=1")).await?;
    assert_eq!(page2, vec![ARTICLE.to_string()]);
    assert_eq!(cursor2, Some(ARTICLE_LATEST_TAG));

    // Offset paging walks the same order
    let (by_offset, _) = get_ids(&format!("{filter}&limit=1&skip=1")).await?;
    assert_eq!(by_offset, vec![ARTICLE.to_string()]);

    // `end` bounds the page from below when descending
    let (bounded, _) = get_ids(&format!("{filter}&limit=10&end={EVENT_LATEST_TAG}")).await?;
    assert_eq!(bounded, vec![EVENT.to_string()]);

    // Ascending reverses the walk
    let (ascending, last) = get_ids(&format!("{filter}&order=ascending&limit=10")).await?;
    assert_eq!(ascending, vec![ARTICLE.to_string(), EVENT.to_string()]);
    assert_eq!(last, Some(EVENT_LATEST_TAG));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resource_ids_empty_filter() -> Result<()> {
    let (ids, last_score) = get_ids("app=nonexistent_app&sorting=timeline").await?;
    assert!(ids.is_empty(), "Non-existent app should return empty");
    assert_eq!(last_score, None);

    Ok(())
}
