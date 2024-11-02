use super::utils::verify_timeline_post_list;
use super::{ROOT_PATH, TAG_LABEL_2, USER_ID};
use crate::service::utils::{make_request, make_wrong_request};
use anyhow::Result;

#[tokio::test]
async fn test_stream_posts_following() -> Result<()> {
    let path = format!("{ROOT_PATH}?viewer_id={}&source=following", USER_ID);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_followers() -> Result<()> {
    let path = format!("{ROOT_PATH}?viewer_id={}&source=followers", USER_ID);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

// ›››››› THE BELLOW REQUESTS HITS THE GRAPH ‹‹‹‹‹‹‹

// Create a generic function to test all the reach endpoints
async fn test_timeline_posts(
    user_id: &str,
    source: &str,
    tags: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    skip: Option<usize>,
    limit: Option<usize>,
    expected_posts: &[&str],
) -> Result<()> {
    let mut path = format!("{ROOT_PATH}?viewer_id={}&source={}", user_id, source);

    if let Some(tags) = tags {
        path.push_str(&format!("&tags={}", tags));
    }
    if let Some(start) = start {
        path.push_str(&format!("&start={}", start));
    }
    if let Some(end) = end {
        path.push_str(&format!("&end={}", end));
    }
    if let Some(skip) = skip {
        path.push_str(&format!("&skip={}", skip));
    }
    if let Some(limit) = limit {
        path.push_str(&format!("&limit={}", limit));
    }

    let body = make_request(&path).await?;
    verify_timeline_post_list(expected_posts.to_vec(), body);

    Ok(())
}

// ##### REACH: FOLLOWING ####
// User from posts.cypher mock
const AMSTERDAM_USER: &str = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";

// Post order by timeline
pub const POST_TA_ING: &str = "A5D6P9V3Q0T";
pub const POST_TB_ING: &str = "C3L7W0F9Q4K8";
pub const POST_TC_ING: &str = "K1P6Q9M2X4J8";
pub const POST_TD_ING: &str = "N7Q2F5W8J0L3";

const START_TIMELINE: &str = "1729308318220";
const END_TIMELINE: &str = "1693824190130";

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_following_with_tag() -> Result<()> {
    test_timeline_posts(
        AMSTERDAM_USER,
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
async fn test_stream_posts_by_timeline_reach_following_with_tag_and_start() -> Result<()> {
    test_timeline_posts(
        AMSTERDAM_USER,
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

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_following_with_tag_start_and_skip() -> Result<()> {
    test_timeline_posts(
        AMSTERDAM_USER,
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

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_following_with_tag_start_skip_and_limit() -> Result<()>
{
    test_timeline_posts(
        AMSTERDAM_USER,
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

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_following_with_tag_and_end() -> Result<()> {
    test_timeline_posts(
        AMSTERDAM_USER,
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

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_following_with_tag_start_and_end() -> Result<()> {
    test_timeline_posts(
        AMSTERDAM_USER,
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
// User from posts.cypher mock
const BOGOTA_USER: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";

// Post order by timeline
pub const POST_TA_ER: &str = "3NFG9K0L5QH4";
pub const POST_TB_ER: &str = "V8N1P3L9J4X0";
pub const POST_TC_ER: &str = "L3W5N0F8Q2J7";
pub const POST_TD_ER: &str = "M4X1P9L2J6K8";

const START_TIMELINE_ER: &str = "1709308315950";
const END_TIMELINE_ER: &str = "1693823567900";

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_followers_with_tag() -> Result<()> {
    test_timeline_posts(
        BOGOTA_USER, 
        "followers", 
        Some(TAG_LABEL_2), 
        None, 
        None, 
        None, 
        None, 
        &[POST_TA_ER, POST_TB_ER, POST_TC_ER, POST_TD_ER]
    ).await
}

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_and_start() -> Result<()> {
    test_timeline_posts(
        BOGOTA_USER, 
        "followers", 
        Some(TAG_LABEL_2), 
        Some(START_TIMELINE_ER), 
        None, 
        None, 
        None, 
        &[POST_TB_ER, POST_TC_ER, POST_TD_ER]
    ).await
}

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_start_and_skip() -> Result<()> {
    test_timeline_posts(
        BOGOTA_USER, 
        "followers", 
        Some(TAG_LABEL_2), 
        Some(START_TIMELINE_ER), 
        None, 
        Some(2), 
        None, 
        &[POST_TD_ER]
    ).await
}

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_start_skip_and_limit() -> Result<()> {
    test_timeline_posts(
        BOGOTA_USER, 
        "followers", 
        Some(TAG_LABEL_2), 
        Some(START_TIMELINE_ER), 
        None, 
        Some(1), 
        Some(1), 
        &[POST_TC_ER]
    ).await
}

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_and_end() -> Result<()> {
    test_timeline_posts(
        BOGOTA_USER, 
        "followers", 
        Some(TAG_LABEL_2), 
        None, 
        Some(END_TIMELINE_ER), 
        None, 
        None, 
        &[POST_TA_ER, POST_TB_ER]
    ).await
}

#[tokio::test]
async fn test_stream_posts_by_timeline_reach_followers_with_tag_start_and_end() -> Result<()> {
    test_timeline_posts(
        BOGOTA_USER, 
        "followers", 
        Some(TAG_LABEL_2), 
        Some(START_TIMELINE_ER), 
        Some(END_TIMELINE_ER), 
        None, 
        None, 
        &[POST_TB_ER]
    ).await
}

// Remaining test cases follow the same pattern.

#[tokio::test]
async fn test_stream_reach_without_viewer_id() -> Result<()> {
    // Missing viewer_id for a reach query should fail
    let path = format!("{ROOT_PATH}?source=following");
    make_wrong_request(&path, Some(400)).await?;

    Ok(())
}

#[tokio::test]
async fn test_stream_invalid_reach() -> Result<()> {
    // Invalid reach value should fail
    let path = format!("{ROOT_PATH}?source=invalid_reach");
    make_wrong_request(&path, Some(400)).await?;

    Ok(())
}
