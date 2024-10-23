use super::ROOT_PATH;
use crate::service::utils::{make_request, make_wrong_request};
use anyhow::Result;

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
    let path = format!("{ROOT_PATH}?sorting=totalengagement");
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
async fn test_stream_invalid_sorting() -> Result<()> {
    // Invalid sorting option should fail
    let endpoint = "/v0/stream/posts?sorting=invalid";
    make_wrong_request(endpoint, Some(400)).await?;

    Ok(())
}
