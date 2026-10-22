use crate::post::{CAIRO_USER, ROOT_PATH};
use crate::utils::{get_request, invalid_get_request, server::TestServiceServer};
use anyhow::Result;
use axum::http::StatusCode;
use deadpool_redis::redis::{self, AsyncCommands};
use nexus_common::db::get_redis_conn;

// Bookmarks their own post
const BOOKMARKER_ID: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
const POST_ID: &str = "2ZCW1TGR5BKG0";

/// The point read serves the graph's bookmark and leaves no copy in Redis.
#[tokio_shared_rt::test(shared)]
async fn test_post_bookmark_is_read_from_the_graph() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let json_key = format!("Bookmark:{BOOKMARKER_ID}:{POST_ID}:{BOOKMARKER_ID}");
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn.del(&json_key).await?;

    let body = get_request(&format!(
        "{ROOT_PATH}/{BOOKMARKER_ID}/{POST_ID}/bookmark?viewer_id={BOOKMARKER_ID}"
    ))
    .await?;
    assert_eq!(body["id"], "2Z9PFGC3WWWW0");
    assert_eq!(body["indexed_at"].as_u64(), Some(1721764200000));

    let refilled: bool = redis_conn.exists(&json_key).await?;
    assert!(!refilled, "a read must not write {json_key}");

    Ok(())
}

/// A bookmark copy left in Redis for a viewer who never bookmarked the post is
/// served by neither the point read nor the post view.
#[tokio_shared_rt::test(shared)]
async fn test_post_bookmark_ignores_a_retired_copy() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let json_key = format!("Bookmark:{BOOKMARKER_ID}:{POST_ID}:{CAIRO_USER}");
    let mut redis_conn = get_redis_conn().await?;
    redis::cmd("JSON.SET")
        .arg(&json_key)
        .arg("$")
        .arg(r#"{"id":"2Z9PFGC3WWWW0","indexed_at":1721764200000}"#)
        .query_async::<()>(&mut redis_conn)
        .await?;

    let post_path = format!("{ROOT_PATH}/{BOOKMARKER_ID}/{POST_ID}");
    let point = invalid_get_request(
        &format!("{post_path}/bookmark?viewer_id={CAIRO_USER}"),
        StatusCode::NOT_FOUND,
    )
    .await;
    let view = get_request(&format!("{post_path}?viewer_id={CAIRO_USER}")).await;
    let _: () = redis_conn.del(&json_key).await?;

    point?;
    let view = view?;
    assert!(
        view["bookmark"].is_null(),
        "the retired copy was served: {}",
        view["bookmark"]
    );

    Ok(())
}
