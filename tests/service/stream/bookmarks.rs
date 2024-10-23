use super::ROOT_PATH;
use crate::service::utils::{make_request, make_wrong_request};
use anyhow::Result;

// User with most bookmarks
const BOOKMARKER_ID: &str = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";

#[tokio::test]
async fn test_stream_bookmarked_posts() -> Result<()> {
    let viewer_id = BOOKMARKER_ID;
    let path = format!("{ROOT_PATH}?viewer_id={}&source=bookmarks", viewer_id);
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
async fn test_stream_bookmarks_without_viewer_id() -> Result<()> {
    // Missing viewer_id for bookmark reach should fail
    let path = format!("{ROOT_PATH}?source=bookmarks");
    make_wrong_request(&path, Some(400)).await?;

    Ok(())
}
