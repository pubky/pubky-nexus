use super::DETROIT;
use crate::stream::post::utils::verify_post_list_kind;
use crate::stream::post::ROOT_PATH;
use crate::stream::post::{AMSTERDAM, BOGOTA};
use crate::utils::get_request;
use anyhow::Result;

const KIND: &str = "file";

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

#[tokio_shared_rt::test(shared)]
async fn test_stream_file_post_kind() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_F1, POST_F2, POST_F3, POST_F4, POST_F5, POST_F6, POST_F7, POST_F8,
    ];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_file_post_kind_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&start={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_F3, POST_F4, POST_F5, POST_F6, POST_F7, POST_F8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_file_post_kind_with_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&end={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_F1, POST_F2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_file_post_kind_with_start_and_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&start={START_TIMELINE}&end={END_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_F3, POST_F4, POST_F5, POST_F6];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_file_post_kind_with_author() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&author_id={BOGOTA}&source=author");

    let body = get_request(&path).await?;
    let post_list = vec![POST_F3, POST_F5, POST_F8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_file_post_kind_with_author_skip_and_limit() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&author_id={BOGOTA}&source=author&skip=1&limit=1");

    let body = get_request(&path).await?;
    let post_list = vec![POST_F5];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FOLLOWERS ####

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_followers() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=followers&observer_id={DETROIT}&kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_F1, POST_F2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

const REACH_START_TIMELINE: &str = "1980477299360";

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_followers_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=followers&observer_id={DETROIT}&kind={KIND}&start={REACH_START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_F2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FOLLOWING ####

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_following() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=following&observer_id={AMSTERDAM}&kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_F3, POST_F4, POST_F5, POST_F6, POST_F7, POST_F8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_following_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=following&observer_id={AMSTERDAM}&kind={KIND}&start=1980477299325"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_F5, POST_F6, POST_F7, POST_F8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FRIENDS ####

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_friends() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=friends&observer_id={DETROIT}&kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_F1, POST_F2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_friends_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=friends&observer_id={DETROIT}&kind={KIND}&start={REACH_START_TIMELINE}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_F2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}
