use super::ROOT_PATH;
use crate::service::utils::make_request;
use anyhow::Result;

#[tokio::test]
async fn test_stream_user_replies() -> Result<()> {
    let author_id = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    let path = format!(
        "{ROOT_PATH}?author_id={}&source=author_replies&sorting=timeline",
        author_id
    );
    let body = make_request(&path).await?;

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
