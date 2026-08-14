use super::utils::{assert_excludes_author, test_reach_filter_with_posts};
use crate::{
    stream::post::{
        utils::{ids_in, verify_post_list},
        AMSTERDAM, BOGOTA, ROOT_PATH, TAG_LABEL_2,
    },
    utils::get_request,
};
use anyhow::Result;

// User from posts.cypher mock
const EIXAMPLE: &str = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";

const START_TIME: usize = 1980477299321;
const END_TIME: usize = 1980477299312;

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_following_excludes_observer_in_matching_window() -> Result<()> {
    // This window contains Amsterdam's 00000039YD99Y / 00000039YD9B2 posts and
    // posts by followed users. Before the fix, the observer injection returned
    // both groups; now only followed-user posts remain.
    let path = format!(
        "{ROOT_PATH}?observer_id={AMSTERDAM}&source=following&viewer_id={AMSTERDAM}&start=1720000000000&end=1690000000000&limit=50"
    );
    let body = get_request(&path).await?;
    let ids = ids_in(&body);

    assert_excludes_author(&body, AMSTERDAM, "following");
    assert!(
        ids.iter().any(|id| id == "00000039YD9C0"),
        "the regression window must retain a known followed-user post, got {ids:?}"
    );
    assert!(
        !ids.iter()
            .any(|id| matches!(id.as_str(), "00000039YD99Y" | "00000039YD9B2")),
        "the regression window must exclude Amsterdam's posts, got {ids:?}"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_following_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={AMSTERDAM}&source=following&viewer_id={AMSTERDAM}&start={START_TIME}&limit=5"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    verify_post_list(
        vec![
            "MLOW1TGL5BKH4",
            "SIJW1TGL5BKG3",
            "GJMW1TGL5BKG3",
            "MLOW1TGL5BKH3",
            "SIJW1TGL5BKG2",
        ],
        body,
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_following_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={AMSTERDAM}&source=following&viewer_id={AMSTERDAM}&start={START_TIME}&end={END_TIME}"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    verify_post_list(
        vec!["MLOW1TGL5BKH4", "SIJW1TGL5BKG3", "GJMW1TGL5BKG3"],
        body,
    );

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

    verify_post_list(vec!["00000039YD9CY", "00000039YD9DA"], body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_followers_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={BOGOTA}&source=followers&viewer_id={BOGOTA}&start={START_TIME_ERS}&end={END_TIME_ERS}"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    verify_post_list(vec!["00000039YD9CY"], body);

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

    verify_post_list(vec!["00000039YD9CY", "00000039YD9DA"], body);

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

    verify_post_list(vec!["00000039YD9CY"], body);

    Ok(())
}

// ›››››› THE BELLOW REQUESTS HITS THE GRAPH ‹‹‹‹‹‹‹

// ##### REACH: FOLLOWING ####

// Post order by timeline
pub const POST_TA_ING: &str = "00000039YD9BM";
pub const POST_TB_ING: &str = "00000039YD9C0";
pub const POST_TC_ING: &str = "00000039YD9CE";
pub const POST_TD_ING: &str = "00000039YD9DP";

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
pub const POST_TA_ER: &str = "00000039YD9B2";
pub const POST_TB_ER: &str = "00000039YD99Y";
pub const POST_TC_ER: &str = "00000039YD9CY";
pub const POST_TD_ER: &str = "00000039YD9DA";

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
pub const POST_TA_FR: &str = "00000039YD9CY";
pub const POST_TB_FR: &str = "00000039YD9DA";
const SELF_FOLLOW_GRAPH_TAG: &str = "reach-self-follow";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_timeline_reach_friends_excludes_observer() -> Result<()> {
    // Both Eixample and its friend Detroit have a post carrying this tag.
    // Eixample's self-follow must not make its own post part of the friends stream.
    test_reach_filter_with_posts(
        EIXAMPLE,
        None,
        "friends",
        Some(SELF_FOLLOW_GRAPH_TAG),
        None,
        None,
        None,
        None,
        &[POST_TA_FR],
    )
    .await
}

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
