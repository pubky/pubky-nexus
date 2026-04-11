use crate::utils::get_request;
use anyhow::Result;

// Note: Stream tests may read from Redis (populated by watcher tests) OR Neo4j (seed data).

const ROOT_PATH: &str = "/v0/stream/resources";
const IDS_PATH: &str = "/v0/stream/resources/ids";

// =============================================
// GET /v0/stream/resources (returns Vec<ResourceView>)
// =============================================

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_all() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline");
    let body = get_request(&path).await?;

    // Response is Vec<ResourceView>. May be empty if Redis has ghost IDs
    // from previous watcher tests that no longer exist in Neo4j.
    assert!(body.is_array(), "Should return array of ResourceView");
    let views = body.as_array().expect("Should be array");

    // Verify ResourceView structure if any returned
    for view in views {
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

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_by_app_mapky() -> Result<()> {
    let path = format!("{ROOT_PATH}?app=mapky&sorting=timeline");
    let body = get_request(&path).await?;

    let views = body.as_array().expect("Should be array");
    assert!(
        !views.is_empty(),
        "Mapky app filter should return resources"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_by_app_eventky() -> Result<()> {
    let path = format!("{ROOT_PATH}?app=eventky&sorting=timeline");
    let body = get_request(&path).await?;

    let views = body.as_array().expect("Should be array");
    // Eventky data may only exist in Neo4j seed (not Redis), so may be empty
    // if Redis has data from watcher tests that shadows the fallback.
    // Just verify valid response structure.
    for view in views {
        assert!(view["details"].is_object(), "Should have details");
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_by_tag_bitcoin() -> Result<()> {
    let path = format!("{ROOT_PATH}?tags=bitcoin&sorting=timeline");
    let body = get_request(&path).await?;

    let views = body.as_array().expect("Should be array");
    assert!(
        !views.is_empty(),
        "Bitcoin tag filter should return resources"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_combined_app_and_tag() -> Result<()> {
    let path = format!("{ROOT_PATH}?app=mapky&tags=bitcoin&sorting=timeline");
    let body = get_request(&path).await?;

    let views = body.as_array().expect("Should be array");
    assert!(
        !views.is_empty(),
        "Mapky+bitcoin combined filter should return resources"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_sorting_taggers_count() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=taggers_count");
    let body = get_request(&path).await?;

    let views = body.as_array().expect("Should be array");
    assert!(
        !views.is_empty(),
        "Should return resources sorted by taggers count"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resources_pagination() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline&limit=1");
    let body = get_request(&path).await?;

    let views = body.as_array().expect("Should be array");
    // limit=1 on IDs, but ResourceView loading may skip ghost IDs not in Neo4j
    assert!(
        views.len() <= 1,
        "Should respect limit=1 (may be 0 if ID is stale)"
    );

    Ok(())
}

// =============================================
// GET /v0/stream/resources/ids (returns ResourceKeyStream)
// =============================================

#[tokio_shared_rt::test(shared)]
async fn test_stream_resource_ids() -> Result<()> {
    let path = format!("{IDS_PATH}?sorting=timeline");
    let body = get_request(&path).await?;

    assert!(body["resource_ids"].is_array(), "Should have resource_ids");
    let ids = body["resource_ids"].as_array().expect("Should be array");
    assert!(!ids.is_empty(), "Should return resources");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_resource_ids_empty_filter() -> Result<()> {
    let path = format!("{IDS_PATH}?app=nonexistent_app&sorting=timeline");
    let body = get_request(&path).await?;

    let ids = body["resource_ids"].as_array().expect("Should be array");
    assert_eq!(ids.len(), 0, "Non-existent app should return empty");

    Ok(())
}
