use crate::service::utils::{make_request, make_wrong_request};
use pubky_nexus::models::post::PostStream;
use anyhow::Result;

use super::utils::search_tag_in_post;
use super::{POST_A, POST_B, POST_C, POST_F, POST_G, POST_H};
use super::{ROOT_PATH, USER_ID};
use super::{TAG_LABEL_1, TAG_LABEL_2};

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

#[tokio::test]
async fn test_post_tag_search_by_engagement() -> Result<()> {
    let post_order = vec![POST_A, POST_C, POST_B, POST_H];
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
    let post_order = vec![POST_C];
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
    let viewer_id = USER_ID;
    let tag = TAG_LABEL_1;
    let path = format!(
        "{ROOT_PATH}?viewer_id={}&source=following&tags={}&sorting=total_engagement",
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


#[tokio::test]
async fn test_stream_invalid_sorting() -> Result<()> {
    // Invalid sorting option should fail
    let endpoint = "/v0/stream/posts?sorting=invalid";
    make_wrong_request(endpoint, Some(400)).await?;

    Ok(())
}
