use super::{ROOT_PATH, USER_ID};
use crate::service::utils::{make_request, make_wrong_request};
use anyhow::Result;

#[tokio::test]
async fn test_stream_posts_following() -> Result<()> {
    let viewer_id = USER_ID;
    let path = format!("{ROOT_PATH}?viewer_id={}&source=following", viewer_id);
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
    let viewer_id = USER_ID;
    let path = format!("{ROOT_PATH}?viewer_id={}&source=followers", viewer_id);
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
