use crate::utils::{get_request, invalid_get_request, post_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::models::post::PostStream;
use serde_json::json;

use super::utils::{search_tag_in_post, verify_post_list, verify_timeline_post_list};
use super::{POST_A, POST_B, POST_C, POST_F, POST_G, POST_H};
use super::{ROOT_PATH, USER_ID};
use super::{TAG_LABEL_1, TAG_LABEL_2};

const CAIRO_USER: &str = "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o";

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

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_global_timeline() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline");
    let body = get_request(&path).await?;

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

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_global_timeline_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline&start={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_TA, POST_TB, POST_TC, POST_TD, POST_TE, POST_TF, POST_TG, POST_TH, POST_TI, POST_TJ,
    ];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_global_timeline_with_start_and_limit() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline&start={START_TIMELINE}&limit=5");

    let body = get_request(&path).await?;
    let post_list = vec![POST_TA, POST_TB, POST_TC, POST_TD, POST_TE];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_global_timeline_with_start_and_limit_and_skip() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline&start={START_TIMELINE}&skip=3&limit=5");

    let body = get_request(&path).await?;
    let post_list = vec![POST_TD, POST_TE, POST_TF, POST_TG, POST_TH];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_global_total_engagement() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=total_engagement");
    let body = get_request(&path).await?;

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
pub const POST_EI: &str = "0032GZQ338BMP";
pub const POST_EJ: &str = "2Z1N8QBETDS00";
pub const POST_EK: &str = "2Z1N8QBERHB00";
pub const POST_EL: &str = "2Z1N8QBETHK00";
pub const POST_TAG_ME: &str = "0032BZ0T19R70";

pub const POST_E0: &str = "2Z1PBYS0F90G0";
pub const POST_E1: &str = "2ZECRNM66G900";
pub const POST_00: &str = "2Z1P68V42JJ00";

pub const ENGAGEMENT_SCORE: &str = "10";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_global_total_engagement_with_start_score() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=total_engagement&start={ENGAGEMENT_SCORE}");

    let body = get_request(&path).await?;

    let post_list = vec![
        POST_EA, POST_EB, POST_EC, POST_ED, POST_EF, POST_EG, POST_EH, POST_EI, POST_EJ, POST_EK,
    ];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_global_total_engagement_with_start_end_score() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?sorting=total_engagement&start={ENGAGEMENT_SCORE}&end={ENGAGEMENT_SCORE}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_EA, POST_EB, POST_EC];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_global_total_engagement_with_end_score() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=total_engagement&end={ENGAGEMENT_SCORE}");

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_TAG_ME,
        POST_00,
        POST_E0,
        POST_E1,
        POST_EA,
        POST_EB,
        POST_EC,
    ];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_tag_search_by_engagement() -> Result<()> {
    let post_order = vec![POST_A, POST_H, POST_C, POST_B];
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_2}&sorting=total_engagement&limit=4");

    let body = get_request(&path).await?;
    assert!(body.is_array());
    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 4);
    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_tag_search_by_engagement_with_skip() -> Result<()> {
    let post_order = vec![POST_G, POST_F];
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_2}&sorting=total_engagement&skip=6");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_tag_search_by_engagement_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_H];
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_2}&sorting=total_engagement&skip=1&limit=1");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_combined_parameters() -> Result<()> {
    // This one should hit the graph
    let observer_id = USER_ID;
    let tag = TAG_LABEL_1;
    let path = format!(
        "{ROOT_PATH}?observer_id={observer_id}&source=following&tags={tag}&sorting=total_engagement"
    );

    let body = get_request(&path).await?;

    // Deserialize the response body into a PostStream object
    let post_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(!post_stream.0.is_empty(), "Post stream should not be empty");

    // Iterate over each post and check if it contains the requested tag
    for post in post_stream.0 {
        let has_tag = post.tags.iter().any(|tag_item| tag_item.label == tag); // Use iterators to check if any tag matches the label

        assert!(
            has_tag,
            "Post should be tagged with the requested tag: {tag}"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_invalid_sorting() -> Result<()> {
    // Invalid sorting option should fail
    let endpoint = "/v0/stream/posts?sorting=invalid";
    invalid_get_request(endpoint, StatusCode::BAD_REQUEST).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_with_attachment_metadata() -> Result<()> {
    // Stream Cairo user's posts with attachment metadata
    let path = format!(
        "{ROOT_PATH}?author_id={CAIRO_USER}&source=author&sorting=timeline&include_attachment_metadata=true&limit=20"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());
    let posts = body.as_array().expect("Post stream should be an array");

    // Find the post with attachments (POST_H)
    let post_with_attachments = posts
        .iter()
        .find(|p| p["details"]["id"] == POST_H)
        .expect("POST_H should be in the stream");

    let attachments_metadata = post_with_attachments["attachments_metadata"]
        .as_array()
        .expect("Post attachments_metadata should be an array");
    assert_eq!(attachments_metadata.len(), 2);

    assert_eq!(attachments_metadata[0]["id"], "2ZK3A1B2C3D40");
    assert_eq!(attachments_metadata[0]["owner_id"], CAIRO_USER);
    assert_eq!(attachments_metadata[0]["name"], "cairo_file1");
    assert_eq!(attachments_metadata[0]["content_type"], "image/png");

    assert_eq!(attachments_metadata[1]["id"], "2ZK3E5F6G7H80");
    assert_eq!(attachments_metadata[1]["owner_id"], CAIRO_USER);
    assert_eq!(attachments_metadata[1]["name"], "cairo_file2");
    assert_eq!(attachments_metadata[1]["content_type"], "image/jpeg");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_without_attachment_metadata() -> Result<()> {
    // Without the flag, posts should not have attachments_metadata
    let path =
        format!("{ROOT_PATH}?author_id={CAIRO_USER}&source=author&sorting=timeline&limit=20");
    let body = get_request(&path).await?;

    assert!(body.is_array());
    let posts = body.as_array().expect("Post stream should be an array");

    let post_with_attachments = posts
        .iter()
        .find(|p| p["details"]["id"] == POST_H)
        .expect("POST_H should be in the stream");

    // attachments_metadata should be absent (skipped via skip_serializing_if when empty)
    assert!(
        post_with_attachments.get("attachments_metadata").is_none(),
        "attachments_metadata should be absent when flag is not set"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_ids_with_attachment_metadata() -> Result<()> {
    let post_key = format!("{CAIRO_USER}:{POST_H}");
    let request_body = json!({
        "post_ids": [post_key],
        "include_attachment_metadata": true
    });

    let body = post_request("/v0/stream/posts/by_ids", request_body).await?;

    assert!(body.is_array());
    let posts = body.as_array().expect("Post stream should be an array");
    assert_eq!(posts.len(), 1);

    let attachments_metadata = posts[0]["attachments_metadata"]
        .as_array()
        .expect("Post attachments_metadata should be an array");
    assert_eq!(attachments_metadata.len(), 2);

    assert_eq!(attachments_metadata[0]["id"], "2ZK3A1B2C3D40");
    assert_eq!(attachments_metadata[1]["id"], "2ZK3E5F6G7H80");

    Ok(())
}
