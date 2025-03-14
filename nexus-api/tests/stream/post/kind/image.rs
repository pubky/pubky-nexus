use super::DETROIT;
use crate::stream::post::utils::verify_post_list_kind;
use crate::stream::post::ROOT_PATH;
use crate::stream::post::{AMSTERDAM, BOGOTA};
use crate::utils::get_request;
use anyhow::Result;

const KIND: &str = "image";

const POST_I1: &str = "5YCW1TGL5BKG8";
const POST_I2: &str = "5YCW1TGL5BKG7";
const POST_I3: &str = "5YCW1TGL5BKG6";
const POST_I4: &str = "5YCW1TGL5BKG5";
const POST_I5: &str = "5YCW1TGL5BKG4";
const POST_I6: &str = "5YCW1TGL5BKG3";
const POST_I7: &str = "5YCW1TGL5BKG2";
const POST_I8: &str = "5YCW1TGL5BKG1";

pub const START_TIMELINE: &str = "1820477299345";
pub const END_TIMELINE: &str = "1820477299308";

#[tokio_shared_rt::test(shared)]
async fn test_stream_image_post_kind() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_I1, POST_I2, POST_I3, POST_I4, POST_I5, POST_I6, POST_I7, POST_I8,
    ];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_image_post_kind_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&start={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_I3, POST_I4, POST_I5, POST_I6, POST_I7, POST_I8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_image_post_kind_with_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&end={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_I1, POST_I2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_image_post_kind_with_start_and_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&start={START_TIMELINE}&end={END_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_I3, POST_I4, POST_I5, POST_I6];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_image_post_kind_with_author() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&author_id={BOGOTA}&source=author");

    let body = get_request(&path).await?;
    let post_list = vec![POST_I3, POST_I5, POST_I8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_image_post_kind_with_author_skip_and_limit() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&author_id={BOGOTA}&source=author&skip=1&limit=1");

    let body = get_request(&path).await?;
    let post_list = vec![POST_I5];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FOLLOWING ####

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_followers() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=followers&observer_id={DETROIT}&kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_I1, POST_I2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

const REACH_START_TIMELINE: &str = "1820477299355";

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_followers_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=followers&observer_id={DETROIT}&kind={KIND}&start={REACH_START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_I2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FOLLOWING ####

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_following() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=following&observer_id={AMSTERDAM}&kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_I3, POST_I4, POST_I5, POST_I6, POST_I7, POST_I8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_following_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=following&observer_id={AMSTERDAM}&kind={KIND}&start=1820477299325"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_I5, POST_I6, POST_I7, POST_I8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FRIENDS ####

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_friends() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=friends&observer_id={DETROIT}&kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_I1, POST_I2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_friends_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=friends&observer_id={DETROIT}&kind={KIND}&start={REACH_START_TIMELINE}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_I2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}
