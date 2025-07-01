use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::models::post::PostStream;

use super::utils::{search_tag_in_post, verify_post_list, verify_timeline_post_list};
use super::{POST_A, POST_B, POST_C, POST_D, POST_E, POST_F, POST_G, POST_H};
use super::{ROOT_PATH, VIEWER_ID};
use super::{TAG_LABEL_1, TAG_LABEL_2, TAG_LABEL_3, TAG_LABEL_4};

#[tokio_shared_rt::test(shared)]
async fn test_post_tag_search() -> Result<()> {
    let post_order = vec![POST_C, POST_B, POST_A, POST_D, POST_E, POST_F];
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_2}&limit=6");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 6);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_wrong_tag_param() -> Result<()> {
    let path = format!("{ROOT_PATH}?tags=");
    invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_tag_search_with_limit() -> Result<()> {
    let post_order = vec![POST_C, POST_B];
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_2}&limit=2");
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
async fn test_post_tag_search_with_skip() -> Result<()> {
    let post_order = vec![POST_G, POST_H];
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_2}&skip=6");
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
async fn test_post_tag_search_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_B];
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_2}&skip=1&limit=1");
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
async fn test_post_tag_search_with_viewer_id() -> Result<()> {
    const BOOKMARK_ID: &str = "A9G7F2L4Q1W3";

    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_2}&viewer_id={VIEWER_ID}");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 8);

    assert_eq!(tags[0]["bookmark"]["id"], BOOKMARK_ID);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_tag() -> Result<()> {
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=timeline");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    // Deserialize the response body into a PostStream object
    let post_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(!post_stream.0.is_empty(), "Post stream should not be empty");

    // Iterate over each post and check if it contains the requested tag
    for post in post_stream.0 {
        let has_tag = post.tags.iter().any(|tag| tag.label == TAG_LABEL_1);

        assert!(
            has_tag,
            "Post should be tagged with the requested tag: {TAG_LABEL_1}"
        );
    }
    Ok(())
}

// Post tags order by timeline
pub const POST_TA: &str = "2ZGJQG7Z757G0";
pub const POST_TB: &str = "2ZGJQDXMHRH00";
pub const POST_TC: &str = "2ZDZR2G775W00";
pub const POST_TD: &str = "2ZDZK595DDRG0";
pub const POST_TE: &str = "2ZDZHGVTQV600";
pub const POST_TF: &str = "2ZDZ7PM0JVK00";
pub const POST_TG: &str = "2ZDZ4DTKRJ900";
pub const POST_TH: &str = "2ZDZ2SF29HK00";
pub const POST_TI: &str = "2ZDYZGQ4XKTG0";
pub const POST_TJ: &str = "2ZDYXW8751NG0";
pub const POST_TK: &str = "2ZDYQAQFA74G0";
pub const POST_TL: &str = "2ZDYGS5S86D00";
pub const POST_TM: &str = "2ZDYA7MH312G0";

pub const START_TIMELINE: &str = "1719244802772";
pub const END_TIMELINE: &str = "1719231303114";

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_tag_timeline_with_start() -> Result<()> {
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=timeline&start={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_TD, POST_TE, POST_TF, POST_TG, POST_TH, POST_TI, POST_TJ, POST_TK, POST_TL, POST_TM,
    ];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_tag_timeline_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=timeline&start={START_TIMELINE}&end={END_TIMELINE}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_TD, POST_TE, POST_TF, POST_TG, POST_TH, POST_TI];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_tag_timeline_with_end() -> Result<()> {
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=timeline&end={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![POST_TA, POST_TB, POST_TC];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_tag_timeline_with_end_and_skip() -> Result<()> {
    let path =
        format!("{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=timeline&end={START_TIMELINE}&skip=2");

    let body = get_request(&path).await?;
    let post_list = vec![POST_TC];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_tag_timeline_with_start_skip_and_limit() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=timeline&start={START_TIMELINE}&skip=2&limit=5"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_TF, POST_TG, POST_TH, POST_TI, POST_TJ];

    verify_timeline_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_posts_by_multiple_tags() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?tags={TAG_LABEL_2},{TAG_LABEL_3},{TAG_LABEL_4}&sorting=timeline&limit=30"
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    // Deserialize the response body into a PostStream object
    let post_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(!post_stream.0.is_empty(), "Post stream should not be empty");

    // Define the set of tags you want to check
    let valid_tags = vec![
        TAG_LABEL_2.to_owned(),
        TAG_LABEL_3.to_owned(),
        TAG_LABEL_4.to_owned(),
    ];

    // Iterate over each post and check if it contains any of the requested tags
    for post in post_stream.0 {
        let has_tag = post.tags.iter().any(|tag| valid_tags.contains(&tag.label));

        assert!(
            has_tag,
            "Post should be tagged with any of the requested tags: {valid_tags:?}"
        );
    }

    Ok(())
}

// Post tags order by engagment
pub const POST_EA: &str = "2Z8W2AFP242G0";
pub const POST_EB: &str = "2Z9GWEBYKY400";
pub const POST_EC: &str = "2ZAV28YDJSXG0";
pub const POST_ED: &str = "2ZDRZZCEDAF00";
pub const POST_EE: &str = "2ZDPHVBK54XG0";
pub const POST_EF: &str = "2ZAVFC1DZHPG0";
pub const POST_EG: &str = "2ZAV8TGM8QB00";
pub const POST_EH: &str = "2ZDZHGVTQV600";
pub const POST_EI: &str = "2ZDZ4DTKRJ900";
pub const POST_EJ: &str = "2ZDZ2SF29HK00";
pub const POST_EK: &str = "2ZDYQAQFA74G0";
pub const POST_EL: &str = "2ZDYGS5S86D00";

pub const ENGAGEMENT_SCORE_START: &str = "6";
pub const ENGAGEMENT_SCORE_END: &str = "4";

#[tokio_shared_rt::test(shared)]
async fn test_stream_tag_posts_by_engagment_tag() -> Result<()> {
    let path = format!("{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=total_engagement");

    let body = get_request(&path).await?;

    // Deserialize the response body into a PostStream object
    let post_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(!post_stream.0.is_empty(), "Post stream should not be empty");

    // Iterate over each post and check if it contains the requested tag
    for post in post_stream.0 {
        let has_tag = post.tags.iter().any(|tag| tag.label == TAG_LABEL_1);

        assert!(
            has_tag,
            "Post should be tagged with the requested tag: {TAG_LABEL_1}"
        );
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_tag_posts_by_engagement_with_start() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=total_engagement&start={ENGAGEMENT_SCORE_START}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_EB, POST_EC, POST_ED, POST_EE, POST_EF, POST_EG, POST_EH, POST_EI, POST_EJ, POST_EK,
    ];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_tag_posts_by_engagement_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=total_engagement&start={ENGAGEMENT_SCORE_START}&end={ENGAGEMENT_SCORE_END}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_EB, POST_EC, POST_ED, POST_EE, POST_EF, POST_EG];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_tag_posts_by_engagement_with_start_and_limit() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=total_engagement&start={ENGAGEMENT_SCORE_END}&limit=6"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_ED, POST_EE, POST_EF, POST_EG, POST_EH, POST_EI];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_tag_posts_by_engagement_with_end_skip_and_limit() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?tags={TAG_LABEL_1}&sorting=total_engagement&end={ENGAGEMENT_SCORE_END}&skip=3&limit=6"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_ED, POST_EE, POST_EF, POST_EG];

    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag_with_no_result() -> Result<()> {
    let path = format!("{}?tags={}", ROOT_PATH, "randommm");
    invalid_get_request(&path, StatusCode::NO_CONTENT).await?;

    Ok(())
}
