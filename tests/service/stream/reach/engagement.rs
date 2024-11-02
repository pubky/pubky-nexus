use super::utils::test_reach_filter_with_posts;
use crate::service::stream::TAG_LABEL_2;
use anyhow::Result;

// ›››››› THE BELLOW REQUESTS HITS THE GRAPH ‹‹‹‹‹‹‹

// ##### REACH: FOLLOWING ####
// User from posts.cypher mock
const AMSTERDAM_USER: &str = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";

// Post order by timeline
pub const POST_TA_ING: &str = "N7Q2F5W8J0L3";
pub const POST_TB_ING: &str = "A5D6P9V3Q0T";
pub const POST_TC_ING: &str = "K1P6Q9M2X4J8";
pub const POST_TD_ING: &str = "C3L7W0F9Q4K8";

const START_SCORE: &str = "5";

#[tokio::test]
async fn test_stream_posts_by_engagement_reach_following_with_tag() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM_USER,
        Some("total_engagement"),
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

#[tokio::test]
async fn test_stream_posts_by_engagement_reach_following_with_tag_and_start() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM_USER,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        Some(START_SCORE),
        None,
        None,
        None,
        &[POST_TB_ING, POST_TC_ING, POST_TD_ING],
    )
    .await
}

#[tokio::test]
async fn test_stream_posts_by_engagement_reach_following_with_tag_start_and_skip() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM_USER,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        Some(START_SCORE),
        None,
        Some(2),
        None,
        &[POST_TD_ING],
    )
    .await
}

#[tokio::test]
async fn test_stream_posts_by_engagement_reach_following_with_tag_start_skip_and_limit(
) -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM_USER,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        Some(START_SCORE),
        None,
        Some(1),
        Some(1),
        // The POST_TC_ING has the same engagement: 2. For some reason, neo4j filters that node instead of C
        // when we apply the limit CLAUSE
        &[POST_TD_ING],
    )
    .await
}

#[tokio::test]
async fn test_stream_posts_by_engagement_reach_following_with_tag_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM_USER,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        None,
        Some("3"),
        None,
        None,
        &[POST_TA_ING, POST_TB_ING],
    )
    .await
}

#[tokio::test]
async fn test_stream_posts_by_engagement_reach_following_with_tag_start_and_end() -> Result<()> {
    test_reach_filter_with_posts(
        AMSTERDAM_USER,
        Some("total_engagement"),
        "following",
        Some(TAG_LABEL_2),
        Some("4"),
        Some("3"),
        None,
        None,
        &[POST_TB_ING],
    )
    .await
}
