use super::ROOT_PATH;
use crate::utils::get_request;
use anyhow::Result;

#[tokio_shared_rt::test(shared)]
async fn test_stream_user_replies() -> Result<()> {
    let author_id = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    let path = format!("{ROOT_PATH}?author_id={author_id}&source=author_replies&sorting=timeline");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert_eq!(
            post["details"]["author"].as_str(),
            Some(author_id),
            "Post author should match the requested user"
        );
        assert!(
            post["relationships"]["replied"].as_str().is_some(),
            "Posts from reply stream must have a 'replied' relationship"
        );
    }

    Ok(())
}
