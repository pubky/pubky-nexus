use crate::service::stream::utils::verify_post_list;
use crate::service::stream::ROOT_PATH;
use crate::service::utils::make_request;
use anyhow::Result;

const BOGOTA: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";

const POST_F1: &str = "GJMW1TGL5BKG8";
const POST_F2: &str = "GJMW1TGL5BKG7";
const POST_F3: &str = "GJMW1TGL5BKG6";
const POST_F4: &str = "GJMW1TGL5BKG5";
const POST_F5: &str = "GJMW1TGL5BKG4";
const POST_F6: &str = "GJMW1TGL5BKG3";
const POST_F7: &str = "GJMW1TGL5BKG2";
const POST_F8: &str = "GJMW1TGL5BKG1";

pub const START_TIMELINE: &str = "1980477299345";
pub const END_TIMELINE: &str = "1980477299309";

#[tokio::test]
async fn test_stream_file_post_kind() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=file");

    let body = make_request(&path).await?;
    let post_list = vec![
        POST_F1, POST_F2, POST_F3, POST_F4, POST_F5, POST_F6, POST_F7, POST_F8,
    ];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_file_post_kind_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=file&start={START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_F3, POST_F4, POST_F5, POST_F6, POST_F7, POST_F8];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_file_post_kind_with_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=file&end={START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_F1, POST_F2];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_file_post_kind_with_start_and_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=file&start={START_TIMELINE}&end={END_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![POST_F3, POST_F4, POST_F5, POST_F6];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_file_post_kind_with_author() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=file&author_id={BOGOTA}&source=author");

    let body = make_request(&path).await?;
    let post_list = vec![POST_F3, POST_F5, POST_F8];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_file_post_kind_with_author_skip_and_limit() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind=file&author_id={BOGOTA}&source=author&skip=1&limit=1");

    let body = make_request(&path).await?;
    let post_list = vec![POST_F5];
    verify_post_list(post_list, body);

    Ok(())
}
