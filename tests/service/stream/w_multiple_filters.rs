use crate::service::utils::make_request;
use anyhow::Result;
use pubky_nexus::models::post::PostStream;

use super::utils::search_tag_in_post;
use super::{POST_A, POST_B, POST_C, POST_F, POST_G, POST_H};
use super::{ROOT_PATH, USER_ID, VIEWER_ID};
use super::{TAG_LABEL_1, TAG_LABEL_2};

const BOOKMARK_ID: &str = "A9G7F2L4Q1W3";

#[tokio::test]
async fn test_post_tag_search_with_viewer_id() -> Result<()> {
    let path = format!("{}?tags={}&viewer_id={}", ROOT_PATH, TAG_LABEL_2, VIEWER_ID);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 8);

    assert_eq!(tags[0]["bookmark"]["id"], BOOKMARK_ID);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement() -> Result<()> {
    let post_order = vec![POST_A, POST_C, POST_B, POST_H];
    let path = format!(
        "{}?tags={}&sorting=totalengagement&limit=4",
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
        "{}?tags={}&sorting=totalengagement&skip=6",
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
    let post_order = vec![POST_C];
    let path = format!(
        "{}?tags={}&sorting=totalengagement&skip=1&limit=1",
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
    let viewer_id = USER_ID;
    let tag = TAG_LABEL_1;
    let path = format!(
        "{ROOT_PATH}?viewer_id={}&source=following&tags={}&sorting=totalengagement",
        viewer_id, tag
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
