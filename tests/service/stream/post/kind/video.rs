use super::DETROIT;
use crate::service::stream::post::utils::verify_post_list_kind;
use crate::service::stream::post::ROOT_PATH;
use crate::service::stream::post::{AMSTERDAM, BOGOTA};
use crate::service::utils::make_request;
use anyhow::Result;

const KIND: &str = "video";

const POST_V1: &str = "MLOW1TGL5BKH8";
const POST_V2: &str = "MLOW1TGL5BKH7";
const POST_V3: &str = "MLOW1TGL5BKH6";
const POST_V4: &str = "MLOW1TGL5BKH5";
const POST_V5: &str = "MLOW1TGL5BKH4";
const POST_V6: &str = "MLOW1TGL5BKH3";
const POST_V7: &str = "MLOW1TGL5BKH2";
const POST_V8: &str = "MLOW1TGL5BKH1";

pub const START_TIMELINE: &str = "1980477299345";
pub const END_TIMELINE: &str = "1980477299308";

#[tokio::test]
async fn test_stream_video_post_kind() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}");

    let body = make_request(&path).await?;
    let post_list = vec![
        POST_V1, POST_V2, POST_V3, POST_V4, POST_V5, POST_V6, POST_V7, POST_V8,
    ];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&start={START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V3, POST_V4, POST_V5, POST_V6, POST_V7, POST_V8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&end={START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V1, POST_V2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_start_and_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&start={START_TIMELINE}&end={END_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V3, POST_V4, POST_V5, POST_V6];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_author() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&author_id={BOGOTA}&source=author");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V3, POST_V5, POST_V8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_author_skip_and_limit() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&author_id={BOGOTA}&source=author&skip=1&limit=1");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V5];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FOLLOWERS ####

#[tokio::test]
async fn test_stream_post_kind_followers() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=followers&observer_id={DETROIT}&kind={KIND}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V1, POST_V2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

const REACH_START_TIMELINE: &str = "1980477299355";

#[tokio::test]
async fn test_stream_post_kind_followers_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=followers&observer_id={DETROIT}&kind={KIND}&start={REACH_START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FOLLOWING ####

#[tokio::test]
async fn test_stream_post_kind_following() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=following&observer_id={AMSTERDAM}&kind={KIND}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V3, POST_V4, POST_V5, POST_V6, POST_V7, POST_V8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio::test]
async fn test_stream_post_kind_following_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=following&observer_id={AMSTERDAM}&kind={KIND}&start=1980477299325"
    );

    let body = make_request(&path).await?;
    let post_list = vec![POST_V5, POST_V6, POST_V7, POST_V8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FRIENDS ####

#[tokio::test]
async fn test_stream_post_kind_friends() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=friends&observer_id={DETROIT}&kind={KIND}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V1, POST_V2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio::test]
async fn test_stream_post_kind_friends_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=friends&observer_id={DETROIT}&kind={KIND}&start={REACH_START_TIMELINE}"
    );

    let body = make_request(&path).await?;
    let post_list = vec![POST_V2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}
