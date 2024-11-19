use crate::service::stream::utils::verify_post_list;
use crate::service::stream::ROOT_PATH;
use crate::service::utils::make_request;
use anyhow::Result;

const BOGOTA: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";

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
    let path = format!("{ROOT_PATH}?kind=video");

    let body = make_request(&path).await?;
    let post_list = vec![
        POST_V1, POST_V2, POST_V3, POST_V4, POST_V5, POST_V6, POST_V7, POST_V8,
    ];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=video&start={START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V3, POST_V4, POST_V5, POST_V6, POST_V7, POST_V8];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=video&end={START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V1, POST_V2];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_start_and_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=video&start={START_TIMELINE}&end={END_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V3, POST_V4, POST_V5, POST_V6];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_author() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=video&author_id={BOGOTA}&source=author");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V3, POST_V5, POST_V8];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_video_post_kind_with_author_skip_and_limit() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=video&author_id={BOGOTA}&source=author&skip=1&limit=1");

    let body = make_request(&path).await?;
    let post_list = vec![POST_V5];
    verify_post_list(post_list, body);

    Ok(())
}
