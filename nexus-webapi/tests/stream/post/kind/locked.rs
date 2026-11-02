//! `locked` filter: server-side lock filtering on post streams.

use super::EIXAMPLE;
use crate::stream::post::utils::verify_post_list;
use crate::stream::post::{AMSTERDAM, BOGOTA, ROOT_PATH};
use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;

const LOCKED_LINK_BOGOTA: &str = "SIJW1TGL5BKG6";
const LOCKED_IMAGE_EIXAMPLE: &str = "5YCW1TGL5BKG8";
const LOCKED_REPLY_BOGOTA: &str = "0000003A0BJWT";

#[tokio_shared_rt::test(shared)]
async fn test_stream_locked_posts_of_any_kind() -> Result<()> {
    let body = get_request(&format!("{ROOT_PATH}?locked=true")).await?;
    verify_post_list(vec![LOCKED_LINK_BOGOTA, LOCKED_IMAGE_EIXAMPLE], body);
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
