use super::DETROIT;
use crate::stream::post::utils::verify_post_list_kind;
use crate::stream::post::ROOT_PATH;
use crate::stream::post::{AMSTERDAM, BOGOTA};
use crate::utils::get_request;
use anyhow::Result;

const KIND: &str = "link";

const POST_L1: &str = "SIJW1TGL5BKG8";
const POST_L2: &str = "SIJW1TGL5BKG7";
const POST_L3: &str = "SIJW1TGL5BKG6";
const POST_L4: &str = "SIJW1TGL5BKG5";
const POST_L5: &str = "SIJW1TGL5BKG4";
const POST_L6: &str = "SIJW1TGL5BKG3";
const POST_L7: &str = "SIJW1TGL5BKG2";
const POST_L8: &str = "SIJW1TGL5BKG1";
const POST_ZERO: &str = "2ZHT82S7G2M00";
const POST_PANDA: &str = "2ZGX2XS9J8R00";
const POST_YT: &str = "2ZG3QX80XEMG0";
const POST_PK: &str = "2ZFWAA83B97G0";

pub const START_TIMELINE: &str = "1980477299350";
pub const END_TIMELINE: &str = "1980477299312";

#[tokio_shared_rt::test(shared)]
async fn test_stream_link_post_kind() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_L1, POST_L2, POST_L3, POST_L4, POST_L5, POST_L6, POST_L7, POST_L8, POST_ZERO,
        POST_PANDA,
    ];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_link_post_kind_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&start={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_L3, POST_L4, POST_L5, POST_L6, POST_L7, POST_L8, POST_ZERO, POST_PANDA, POST_YT,
        POST_PK,
    ];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_link_post_kind_with_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&end={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_L1, POST_L2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_link_post_kind_with_start_and_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&start={START_TIMELINE}&end={END_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_L3, POST_L4, POST_L5, POST_L6];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_link_post_kind_with_author() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&author_id={BOGOTA}&source=author");

    let body = get_request(&path).await?;
    let post_list = vec![POST_L3, POST_L5, POST_L8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_link_post_kind_with_author_skip_and_limit() -> Result<()> {
    let path = format!("{ROOT_PATH}?kind={KIND}&author_id={BOGOTA}&source=author&skip=1&limit=1");

    let body = get_request(&path).await?;
    let post_list = vec![POST_L5];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FOLLOWERS ####

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_followers() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=followers&observer_id={DETROIT}&kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_L1, POST_L2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

const REACH_START_TIMELINE: &str = "1980477299360";

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_followers_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=followers&observer_id={DETROIT}&kind={KIND}&start={REACH_START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_L2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FOLLOWING ####

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_following() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=following&observer_id={AMSTERDAM}&kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_L3, POST_L4, POST_L5, POST_L6, POST_L7, POST_L8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_following_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=following&observer_id={AMSTERDAM}&kind={KIND}&start=1980477299330"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_L5, POST_L6, POST_L7, POST_L8];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

// ##### REACH: FRIENDS ####

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_friends() -> Result<()> {
    let path = format!("{ROOT_PATH}?source=friends&observer_id={DETROIT}&kind={KIND}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_L1, POST_L2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_kind_friends_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=friends&observer_id={DETROIT}&kind={KIND}&start={REACH_START_TIMELINE}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_L2];
    verify_post_list_kind(post_list, body, KIND);

    Ok(())
}
