use super::utils::test_reach_filter_with_posts;
use crate::stream::post::TAG_LABEL_2;
use crate::stream::post::{AMSTERDAM, BOGOTA, ROOT_PATH};
use crate::utils::get_request;
use anyhow::Result;

// ›››››› THE BELLOW REQUESTS HITS THE GRAPH ‹‹‹‹‹‹‹

// ##### REACH: FOLLOWING ####

// Post order by timeline
pub const POST_EA_ING: &str = "N7Q2F5W8J0L3";
pub const POST_EB_ING: &str = "A5D6P9V3Q0T";
pub const POST_EC_ING: &str = "K1P6Q9M2X4J8";
pub const POST_ED_ING: &str = "C3L7W0F9Q4K8";

const START_SCORE: &str = "5";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_following_with_tag() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        None,
        None,
        None,
        None,
        &[POST_EA_ING, POST_EB_ING, POST_EC_ING, POST_ED_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_following_with_tag_and_start() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        Some(START_SCORE),
        None,
        None,
        None,
        &[POST_EB_ING, POST_EC_ING, POST_ED_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_following_with_tag_start_and_skip() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        Some(START_SCORE),
        None,
        Some(2),
        None,
        &[POST_ED_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_following_with_tag_start_skip_and_limit(
) -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        Some(START_SCORE),
        None,
        Some(1),
        Some(1),
        // The POST_TC_ING has the same engagement: 2. For some reason, neo4j filters that node instead of C
        // when we apply the limit CLAUSE
        &[POST_ED_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_following_with_tag_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        None,
        Some("3"),
        None,
        None,
        &[POST_EA_ING, POST_EB_ING],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_following_with_tag_start_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        Some("4"),
        Some("3"),
        None,
        None,
        &[POST_EB_ING],
    )
    .await
}

// ##### REACH: FOLLOWERS ####

// Post order by timeline
pub const POST_EA_ER: &str = "V8N1P3L9J4X0";
pub const POST_EB_ER: &str = "3NFG9K0L5QH4";
pub const POST_EC_ER: &str = "M4X1P9L2J6K8";
pub const POST_ED_ER: &str = "L3W5N0F8Q2J7";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_followers_with_tag() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        Some("total_engagement"),
        "followers",
        Some(TAG_LABEL_2),
        None,
        None,
        None,
        None,
        &[POST_EA_ER, POST_EB_ER, POST_EC_ER, POST_ED_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_followers_with_tag_and_start() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        Some("total_engagement"),
        "followers",
        Some(TAG_LABEL_2),
        Some("5"),
        None,
        None,
        None,
        &[POST_EB_ER, POST_EC_ER, POST_ED_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_followers_with_tag_start_and_skip() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        Some("total_engagement"),
        "followers",
        Some(TAG_LABEL_2),
        Some("3"),
        None,
        Some(1),
        None,
        &[POST_ED_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_followers_with_tag_start_skip_and_limit(
) -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        Some("total_engagement"),
        "followers",
        Some(TAG_LABEL_2),
        Some("3"),
        None,
        Some(1),
        Some(1),
        // The POST_TC_ING has the same engagement: 2. For some reason, neo4j filters that node instead of C
        // when we apply the limit CLAUSE
        &[POST_ED_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_followers_with_tag_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        Some("total_engagement"),
        "followers",
        Some(TAG_LABEL_2),
        None,
        Some("3"),
        None,
        None,
        &[POST_EA_ER, POST_EB_ER],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_followers_with_tag_start_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        BOGOTA,
        Some("total_engagement"),
        "followers",
        Some(TAG_LABEL_2),
        Some("5"),
        Some("4"),
        None,
        None,
        &[POST_EB_ER],
    )
    .await
}

// ##### REACH: FRIENDS ####
// User from posts.cypher mock
const EIXAMPLE: &str = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";

// Post order by timeline
pub const POST_EA_FR: &str = "M4X1P9L2J6K8";
pub const POST_EB_FR: &str = "L3W5N0F8Q2J7";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_engagement_reach_friends_with_tag() -> Result<()> {
    test_reach_filter_with_posts(
        EIXAMPLE,
        Some("total_engagement"),
        "friends",
        Some(TAG_LABEL_2),
        None,
        None,
        None,
        None,
        &[POST_EA_FR, POST_EB_FR],
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_not_found_posts_by_engagement_reach_friends_with_tag() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=total_engagement&tags=opensource&source=friends&observer_id={EIXAMPLE}&skip=2");
    let body = get_request(&path).await?;

    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

    Ok(())
}
