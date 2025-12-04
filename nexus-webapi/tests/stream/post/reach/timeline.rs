use super::utils::test_reach_filter_with_posts;
use crate::{
    stream::post::{AMSTERDAM, BOGOTA, ROOT_PATH, TAG_LABEL_2, USER_ID},
    utils::get_request,
};
use anyhow::Result;

// User from posts.cypher mock
const EIXAMPLE: &str = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_following() -> Result<()> {
    let path = format!("{ROOT_PATH}?observer_id={USER_ID}&source=following");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_followers() -> Result<()> {
    let path = format!("{ROOT_PATH}?observer_id={USER_ID}&source=followers");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

const START_TIME: usize = 1980477299321;
const END_TIME: usize = 1980477299312;

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_following_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={AMSTERDAM}&source=following&viewer_id={AMSTERDAM}&start={START_TIME}&limit=5"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let post_array = [
        "MLOW1TGL5BKH4",
        "SIJW1TGL5BKG3",
        "GJMW1TGL5BKG3",
        "MLOW1TGL5BKH3",
        "SIJW1TGL5BKG2",
    ];

    for (index, post) in body
        .as_array()
        .expect("Post stream should be an array")
        .iter()
        .enumerate()
    {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );

        assert_eq!(
            post_array[index], post["details"]["id"],
            "The post index does not match"
        )
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_following_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={AMSTERDAM}&source=following&viewer_id={AMSTERDAM}&start={START_TIME}&end={END_TIME}"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let post_array = ["MLOW1TGL5BKH4", "SIJW1TGL5BKG3", "GJMW1TGL5BKG3"];

    for (index, post) in body
        .as_array()
        .expect("Post stream should be an array")
        .iter()
        .enumerate()
    {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );

        assert_eq!(
            post_array[index], post["details"]["id"],
            "The post index does not match"
        )
    }

    Ok(())
}

const START_TIME_ERS: usize = 1693824523456;
const END_TIME_ERS: usize = 1693823567880;

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_followers_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={BOGOTA}&source=followers&viewer_id={BOGOTA}&start={START_TIME_ERS}&limit=5"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let post_array = ["K1P6Q9M2X4J8", "L3W5N0F8Q2J7", "M4X1P9L2J6K8"];

    for (index, post) in body
        .as_array()
        .expect("Post stream should be an array")
        .iter()
        .enumerate()
    {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );

        assert_eq!(
            post_array[index], post["details"]["id"],
            "The post index does not match"
        )
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_followers_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={BOGOTA}&source=followers&viewer_id={BOGOTA}&start={START_TIME_ERS}&end={END_TIME_ERS}"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let post_array = ["K1P6Q9M2X4J8", "L3W5N0F8Q2J7"];

    for (index, post) in body
        .as_array()
        .expect("Post stream should be an array")
        .iter()
        .enumerate()
    {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );

        assert_eq!(
            post_array[index], post["details"]["id"],
            "The post index does not match"
        )
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_friend_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={}&source=friends&viewer_id={}&start={}&limit=5",
        EIXAMPLE, EIXAMPLE, "1819477230355"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let post_array = ["4ZCW1TGL5BKG7", "L3W5N0F8Q2J7", "M4X1P9L2J6K8"];

    for (index, post) in body
        .as_array()
        .expect("Post stream should be an array")
        .iter()
        .enumerate()
    {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );

        assert_eq!(
            post_array[index], post["details"]["id"],
            "The post index does not match"
        )
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_friend_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={}&source=friends&viewer_id={}&start={}&end={}",
        EIXAMPLE, EIXAMPLE, "1819477230355", "1693822934570"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let post_array = ["4ZCW1TGL5BKG7", "L3W5N0F8Q2J7"];

    for (index, post) in body
        .as_array()
        .expect("Post stream should be an array")
        .iter()
        .enumerate()
    {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );

        assert_eq!(
            post_array[index], post["details"]["id"],
            "The post index does not match"
        )
    }

    Ok(())
}

// ›››››› THE BELLOW REQUESTS HITS THE GRAPH ‹‹‹‹‹‹‹

// ##### REACH: FOLLOWING ####

// Post order by timeline
pub const POST_TA_ING: &str = "A5D6P9V3Q0T";
pub const POST_TB_ING: &str = "C3L7W0F9Q4K8";
pub const POST_TC_ING: &str = "K1P6Q9M2X4J8";
pub const POST_TD_ING: &str = "N7Q2F5W8J0L3";

const START_TIMELINE: &str = "1729308318220";
const END_TIMELINE: &str = "1693824190130";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_following_with_tag() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        None,
        "following",
        Some(TAG_LABEL_2),
        None,
        None,
        None,
        None,
        &[POST_TA_ING, POST_TB_ING, POST_TC_ING, POST_TD_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_following_with_tag_and_start() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        None,
        "following",
        Some(TAG_LABEL_2),
        Some(START_TIMELINE),
        None,
        None,
        None,
        &[POST_TB_ING, POST_TC_ING, POST_TD_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_following_with_tag_start_and_skip() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        None,
        "following",
        Some(TAG_LABEL_2),
        Some(START_TIMELINE),
        None,
        Some(2),
        None,
        &[POST_TD_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_following_with_tag_start_skip_and_limit() -> Result<()>
{
    test_reach_filter_with_posts(
        AMSTERDAM,
        None,
        "following",
        Some(TAG_LABEL_2),
        Some(START_TIMELINE),
        None,
        Some(1),
        Some(1),
        &[POST_TC_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_following_with_tag_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        None,
        "following",
        Some(TAG_LABEL_2),
        None,
        Some(END_TIMELINE),
        None,
        None,
        &[POST_TA_ING, POST_TB_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_following_with_tag_start_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        None,
        "following",
        Some(TAG_LABEL_2),
        Some(START_TIMELINE),
        Some(END_TIMELINE),
        None,
        None,
        &[POST_TB_ING],
    )
    .await
}

// ##### REACH: FOLLOWERS ####

// Post order by timeline
pub const POST_TA_ER: &str = "3NFG9K0L5QH4";
pub const POST_TB_ER: &str = "V8N1P3L9J4X0";
pub const POST_TC_ER: &str = "L3W5N0F8Q2J7";
pub const POST_TD_ER: &str = "M4X1P9L2J6K8";

const START_TIMELINE_ER: &str = "1709308315950";
const END_TIMELINE_ER: &str = "1693823567900";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_followers_with_tag() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        None,
        "followers",
        Some(TAG_LABEL_2),
        None,
        None,
        None,
        None,
        &[POST_TA_ER, POST_TB_ER, POST_TC_ER, POST_TD_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_and_start() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        None,
        "followers",
        Some(TAG_LABEL_2),
        Some(START_TIMELINE_ER),
        None,
        None,
        None,
        &[POST_TB_ER, POST_TC_ER, POST_TD_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_start_and_skip() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        None,
        "followers",
        Some(TAG_LABEL_2),
        Some(START_TIMELINE_ER),
        None,
        Some(2),
        None,
        &[POST_TD_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_start_skip_and_limit() -> Result<()>
{
    test_reach_filter_with_posts(
        BOGOTA,
        None,
        "followers",
        Some(TAG_LABEL_2),
        Some(START_TIMELINE_ER),
        None,
        Some(1),
        Some(1),
        &[POST_TC_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        None,
        "followers",
        Some(TAG_LABEL_2),
        None,
        Some(END_TIMELINE_ER),
        None,
        None,
        &[POST_TA_ER, POST_TB_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_start_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        None,
        "followers",
        Some(TAG_LABEL_2),
        Some(START_TIMELINE_ER),
        Some(END_TIMELINE_ER),
        None,
        None,
        &[POST_TB_ER],
    )
    .await
}

// ##### REACH: FRIENDS ####

// Post order by timeline
pub const POST_TA_FR: &str = "L3W5N0F8Q2J7";
pub const POST_TB_FR: &str = "M4X1P9L2J6K8";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_friends_with_tag() -> Result<()> {
    test_reach_filter_with_posts(
        EIXAMPLE,
        None,
        "friends",
        Some(TAG_LABEL_2),
        None,
        None,
        None,
        None,
        &[POST_TA_FR, POST_TB_FR],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_not_found_posts_by_timeline_reach_friends_with_tag() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?sorting=timeline&tags=opensource&source=friends&observer_id={EIXAMPLE}&skip=2"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

    Ok(())
}
