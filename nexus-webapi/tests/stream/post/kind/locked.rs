//! `locked` filter: server-side lock filtering on post streams.

use super::EIXAMPLE;
use crate::stream::post::utils::verify_post_list;
use crate::stream::post::{AMSTERDAM, BOGOTA, KEYS_ROOT_PATH, ROOT_PATH};
use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::json;

const LOCKED_LINK_BOGOTA: &str = "SIJW1TGL5BKG6";
const LOCKED_IMAGE_EIXAMPLE: &str = "5YCW1TGL5BKG8";
const LOCKED_REPLY_BOGOTA: &str = "0000003A0BJWT";

// `indexed_at` of the two locked parents.
const LINK_INDEXED_AT: u64 = 1980477299348;
const IMAGE_INDEXED_AT: u64 = 1820477299360;

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_posts_of_any_kind() -> Result<()> {
    let body = get_request(&format!("{ROOT_PATH}?locked=true")).await?;
    verify_post_list(vec![LOCKED_LINK_BOGOTA, LOCKED_IMAGE_EIXAMPLE], body);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_posts_show_their_lock() -> Result<()> {
    let lock = format!("pubky://{BOGOTA}/pub/pubky.app/lock");
    let body = get_request(&format!("{ROOT_PATH}?locked=true")).await?;
    let posts = body.as_array().expect("Post stream should be an array");
    assert_eq!(posts.len(), 2);
    for post in posts {
        assert_eq!(post["details"]["lock"], json!(lock), "{post}");
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_keys_route() -> Result<()> {
    let body = get_request(&format!("{KEYS_ROOT_PATH}?locked=true")).await?;
    assert_eq!(
        body["post_keys"],
        json!([
            format!("{BOGOTA}:{LOCKED_LINK_BOGOTA}"),
            format!("{EIXAMPLE}:{LOCKED_IMAGE_EIXAMPLE}"),
        ])
    );
    assert_eq!(body["last_post_score"], json!(IMAGE_INDEXED_AT));
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_keys_route_rejects_kind_filters_400() -> Result<()> {
    let path = format!("{KEYS_ROOT_PATH}?locked=true&kind=short");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

/// The filter does not depend on who is viewing. A viewer also moves `source=all`
/// onto the path that gives an unranked viewer their own posts back.
#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_is_the_same_for_every_viewer() -> Result<()> {
    for viewer in [BOGOTA, EIXAMPLE, AMSTERDAM] {
        let path = format!("{ROOT_PATH}?locked=true&viewer_id={viewer}");
        verify_post_list(
            vec![LOCKED_LINK_BOGOTA, LOCKED_IMAGE_EIXAMPLE],
            get_request(&path).await?,
        );
    }
    Ok(())
}

/// Neither post has engagement, so the tie falls to `p.id`, descending.
#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_engagement_sort() -> Result<()> {
    let path = format!("{ROOT_PATH}?locked=true&sorting=total_engagement");
    verify_post_list(
        vec![LOCKED_LINK_BOGOTA, LOCKED_IMAGE_EIXAMPLE],
        get_request(&path).await?,
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_pagination() -> Result<()> {
    for (window, expected) in [
        ("limit=1".to_string(), LOCKED_LINK_BOGOTA),
        ("skip=1&limit=1".to_string(), LOCKED_IMAGE_EIXAMPLE),
        (format!("start={IMAGE_INDEXED_AT}"), LOCKED_IMAGE_EIXAMPLE),
        (format!("end={LINK_INDEXED_AT}"), LOCKED_LINK_BOGOTA),
    ] {
        let path = format!("{ROOT_PATH}?locked=true&{window}");
        verify_post_list(vec![expected], get_request(&path).await?);
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_false_applies_no_filter() -> Result<()> {
    let plain = get_request(&format!("{ROOT_PATH}?limit=20")).await?;
    let unfiltered = get_request(&format!("{ROOT_PATH}?limit=20&locked=false")).await?;
    assert_eq!(plain, unfiltered);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_author_source() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=author&author_id={EIXAMPLE}&locked=true");
    verify_post_list(vec![LOCKED_IMAGE_EIXAMPLE], get_request(&path).await?);
    Ok(())
}

/// Amsterdam follows Bogota but not Eixample.
#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_following_source() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=following&observer_id={AMSTERDAM}&locked=true");
    verify_post_list(vec![LOCKED_LINK_BOGOTA], get_request(&path).await?);
    Ok(())
}

/// Bookmarks are the one source that keeps replies.
#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_bookmarks_keep_replies() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=bookmarks&observer_id={EIXAMPLE}&locked=true");
    verify_post_list(vec![LOCKED_REPLY_BOGOTA], get_request(&path).await?);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_rejects_kind_filters_400() -> Result<()> {
    for filter in ["kind=short", "exclude_kinds=collection"] {
        let path = format!("{ROOT_PATH}?locked=true&{filter}");
        invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_rejects_unsupported_sources_400() -> Result<()> {
    let post = LOCKED_LINK_BOGOTA;
    for source in [
        format!("source=post_replies&author_id={BOGOTA}&post_id={post}"),
        format!("source=author_replies&author_id={BOGOTA}"),
        format!("source=collection&author_id={BOGOTA}&post_id={post}"),
        format!("source=post_collections&author_id={BOGOTA}&post_id={post}"),
    ] {
        let path = format!("{ROOT_PATH}?{source}&locked=true");
        invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    }
    Ok(())
}
