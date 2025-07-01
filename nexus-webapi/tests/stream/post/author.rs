use super::{ROOT_PATH, USER_ID};
use crate::utils::get_request;
use anyhow::Result;

#[tokio_shared_rt::test(shared)]
async fn test_stream_user_posts() -> Result<()> {
    let path = format!("{ROOT_PATH}?author_id={USER_ID}&source=author&sorting=timeline");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert_eq!(
            post["details"]["author"].as_str(),
            Some(USER_ID),
            "Post author should match the requested user"
        );
    }

    Ok(())
}
