use crate::utils::get_request;
use anyhow::Result;

use super::{KEYS_ROOT_PATH, ROOT_PATH};

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_keys_align_with_posts() -> Result<()> {
    let keys_path = format!("{KEYS_ROOT_PATH}?sorting=timeline&limit=5");
    let keys_body = get_request(&keys_path).await?;
    assert!(keys_body.is_object(), "Keys response must be an object");

    let posts_path = format!("{ROOT_PATH}?sorting=timeline&limit=5");
    let posts_body = get_request(&posts_path).await?;
    assert!(posts_body.is_array(), "Posts response must be an array");

    let keys = keys_body["post_keys"]
        .as_array()
        .expect("Post key stream should expose a post_keys array");
    assert!(
        keys_body["last_post_score"].is_number() || keys_body["last_post_score"].is_null(),
        "Post key stream should include an optional last_post_score"
    );
    let posts = posts_body
        .as_array()
        .expect("Post stream should be an array");

    assert_eq!(
        keys.len(),
        posts.len(),
        "Post key stream should contain the same number of entries as the post stream",
    );

    for (key_value, post_value) in keys.iter().zip(posts.iter()) {
        let key = key_value
            .as_str()
            .expect("Post key entries should be string values");
        let author_id = post_value["details"]["author"]
            .as_str()
            .expect("Post stream entries should include an author identifier");
        let post_id = post_value["details"]["id"]
            .as_str()
            .expect("Post stream entries should include an id");

        assert_eq!(key, format!("{author_id}:{post_id}"));
    }

    Ok(())
}
