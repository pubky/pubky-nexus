use crate::service::utils::{make_request, make_wrong_request};
use anyhow::Result;
use pubky_nexus::models::post::PostStream;

use super::utils::{search_tag_in_post, verify_post_list, verify_timeline_post_list};
use super::{POST_A, POST_B, POST_C, POST_F, POST_G, POST_H};
use super::{ROOT_PATH, USER_ID};
use super::{TAG_LABEL_1, TAG_LABEL_2};

// Post order by timeline
pub const POST_TA: &str = "2ZKB76Q194T00";
pub const POST_TB: &str = "2ZJQQJ3BKKD00";
pub const POST_TC: &str = "2ZJQQBWW6E600";
pub const POST_TD: &str = "2ZJP575MCRTG0";
pub const POST_TE: &str = "2ZJJJW6THBXG0";
pub const POST_TF: &str = "2ZJJ16FPXTD00";
pub const POST_TG: &str = "2ZJHCZNTJDWG0";
pub const POST_TH: &str = "2ZJ2V2B0YZJ00";
pub const POST_TI: &str = "2ZJ2V0NA6NSG0";
pub const POST_TJ: &str = "2ZHT82S7G2M00";

pub const START_TIMELINE: &str = "1722261385301";

#[tokio::test]
async fn test_stream_posts_global_timeline() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline");
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let mut previous_indexed_at = None;
    for post in body.as_array().expect("Post stream should be an array") {
        let indexed_at = post["details"]["indexed_at"]
            .as_u64()
            .expect("indexed_at should be a valid number");
        if let Some(prev) = previous_indexed_at {
            assert!(indexed_at <= prev, "Posts are not sorted by timeline");
        }
        previous_indexed_at = Some(indexed_at);
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_global_timeline_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline&start={START_TIMELINE}");

    let body = make_request(&path).await?;
    let post_list = vec![
        POST_TA, POST_TB, POST_TC, POST_TD, POST_TE, POST_TF, POST_TG, POST_TH, POST_TI, POST_TJ,
    ];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_global_timeline_with_start_and_limit() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline&start={START_TIMELINE}&limit=5");

    let body = make_request(&path).await?;
    let post_list = vec![POST_TA, POST_TB, POST_TC, POST_TD, POST_TE];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_global_timeline_with_start_and_limit_and_skip() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline&start={START_TIMELINE}&skip=3&limit=5");

    let body = make_request(&path).await?;
    let post_list = vec![POST_TD, POST_TE, POST_TF, POST_TG, POST_TH];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_global_total_engagement() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=total_engagement");
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let mut previous_engagement = None;
    for post in body.as_array().expect("Post stream should be an array") {
        let tags = post["counts"]["tags"]
            .as_u64()
            .expect("tags count should be a number");
        let replies = post["counts"]["replies"]
            .as_u64()
            .expect("replies count should be a number");
        let reposts = post["counts"]["reposts"]
            .as_u64()
            .expect("reposts count should be a number");
        let total_engagement = tags + replies + reposts;

        if let Some(prev) = previous_engagement {
            assert!(
                total_engagement <= prev,
                "Posts are not sorted by total engagement"
            );
        }
        previous_engagement = Some(total_engagement);
    }

    Ok(())
}

// Post order by engagment
pub const POST_EA: &str = "2Z1N8QBESER00";
pub const POST_EB: &str = "2Z1N8QBETW700";
pub const POST_EC: &str = "2Z1N9M56X4EG0";
pub const POST_ED: &str = "2Z1N9M56X7DG0";
pub const POST_EF: &str = "2Z1N9M56W8D00";
pub const POST_EG: &str = "2Z1N9M56X5VG0";
pub const POST_EH: &str = "2Z1N9M56WJKG0";
pub const POST_EI: &str = "2Z1N8QBETDS00";
pub const POST_EJ: &str = "2Z1N8QBERHB00";
pub const POST_EK: &str = "2Z1N8QBETHK00";

pub const POST_E0: &str = "2Z1PBYS0F90G0";
pub const POST_E1: &str = "2ZECRNM66G900";
pub const POST_00: &str = "2Z1P68V42JJ00";

pub const ENGAGEMENT_SCORE: &str = "10";

#[tokio::test]
async fn test_stream_posts_global_total_engagement_with_start_score() -> Result<()> {
    let path = format!(
        "{}?sorting=total_engagement&start={}",
        ROOT_PATH, ENGAGEMENT_SCORE
    );

    let body = make_request(&path).await?;
    let post_list = vec![
        POST_EA, POST_EB, POST_EC, POST_ED, POST_EF, POST_EG, POST_EH, POST_EI, POST_EJ, POST_EK,
    ];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_global_total_engagement_with_start_end_score() -> Result<()> {
    let path = format!(
        "{}?sorting=total_engagement&start={}&end={}",
        ROOT_PATH, ENGAGEMENT_SCORE, ENGAGEMENT_SCORE
    );

    let body = make_request(&path).await?;
    let post_list = vec![POST_EA, POST_EB, POST_EC];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_global_total_engagement_with_end_score() -> Result<()> {
    let path = format!(
        "{}?sorting=total_engagement&end={}",
        ROOT_PATH, ENGAGEMENT_SCORE
    );

    let body = make_request(&path).await?;
    let post_list = vec![POST_00, POST_E0, POST_E1, POST_EA, POST_EB, POST_EC];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement() -> Result<()> {
    let post_order = vec![POST_A, POST_H, POST_C, POST_B];
    let path = format!(
        "{}?tags={}&sorting=total_engagement&limit=4",
        ROOT_PATH, TAG_LABEL_2
    );

    let body = make_request(&path).await?;
    assert!(body.is_array());
    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 4);
    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_skip() -> Result<()> {
    let post_order = vec![POST_G, POST_F];
    let path = format!(
        "{}?tags={}&sorting=total_engagement&skip=6",
        ROOT_PATH, TAG_LABEL_2
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_H];
    let path = format!(
        "{}?tags={}&sorting=total_engagement&skip=1&limit=1",
        ROOT_PATH, TAG_LABEL_2
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_stream_combined_parameters() -> Result<()> {
    // This one should hit the graph
    let observer_id = USER_ID;
    let tag = TAG_LABEL_1;
    let path = format!(
        "{ROOT_PATH}?observer_id={}&source=following&tags={}&sorting=total_engagement",
        observer_id, tag
    );

    let body = make_request(&path).await?;

    // Deserialize the response body into a PostStream object
    let post_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(!post_stream.0.is_empty(), "Post stream should not be empty");

    // Iterate over each post and check if it contains the requested tag
    for post in post_stream.0 {
        let has_tag = post.tags.iter().any(|tag_item| tag_item.label == tag); // Use iterators to check if any tag matches the label

        assert!(
            has_tag,
            "Post should be tagged with the requested tag: {}",
            tag
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_invalid_sorting() -> Result<()> {
    // Invalid sorting option should fail
    let endpoint = "/v0/stream/posts?sorting=invalid";
    make_wrong_request(endpoint, Some(400)).await?;

    Ok(())
}
