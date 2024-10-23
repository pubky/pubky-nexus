use crate::service::utils::{make_request, make_wrong_request};
use anyhow::Result;
use pubky_nexus::models::post::PostStream;

use super::utils::search_tag_in_post;
use super::{POST_A, POST_B, POST_C, POST_D, POST_E, POST_F, POST_G, POST_H};
use super::{ROOT_PATH, TAG_LABEL_1, TAG_LABEL_2, TAG_LABEL_3, TAG_LABEL_4};

#[tokio::test]
async fn test_stream_posts_by_tag() -> Result<()> {
    let path = format!("{ROOT_PATH}?tags={}&sorting=timeline", TAG_LABEL_1);
    let body = make_request(&path).await?;

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
            "Post should be tagged with the requested tag: {}",
            TAG_LABEL_1
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_stream_posts_by_multiple_tags() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?tags={},{},{}&sorting=timeline&limit=30",
        TAG_LABEL_2, TAG_LABEL_3, TAG_LABEL_4
    );
    let body = make_request(&path).await?;

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
            "Post should be tagged with any of the requested tags: {:?}",
            valid_tags
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search() -> Result<()> {
    let post_order = vec![POST_C, POST_B, POST_A, POST_D, POST_E, POST_F];
    let path = format!("{}?tags={}&limit=6", ROOT_PATH, TAG_LABEL_2);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 6);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_limit() -> Result<()> {
    let post_order = vec![POST_C, POST_B];
    let path = format!("{}?tags={}&limit=2", ROOT_PATH, TAG_LABEL_2);
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
async fn test_post_tag_search_with_skip() -> Result<()> {
    let post_order = vec![POST_G, POST_H];
    let path = format!("{}?tags={}&skip=6", ROOT_PATH, TAG_LABEL_2);
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
async fn test_post_tag_search_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_B];
    let path = format!("{}?tags={}&skip=1&limit=1", ROOT_PATH, TAG_LABEL_2);
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
async fn test_post_specific_tag_with_no_result() -> Result<()> {
    let path = format!("{}?tags={}", ROOT_PATH, "randommm");
    make_wrong_request(&path, None).await?;

    Ok(())
}
