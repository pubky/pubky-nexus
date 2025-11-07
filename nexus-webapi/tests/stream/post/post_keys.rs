use crate::utils::get_request;
use anyhow::Result;
use serde_json::Value;

use super::{KEYS_ROOT_PATH, ROOT_PATH, TAG_LABEL_2, TAG_LABEL_3, USER_ID};

/// Helper function to verify that post keys align with posts for a given query
async fn assert_post_keys_align_with_posts(query_params: &str, assertion_msg: &str) -> Result<()> {
    let keys_path = format!("{KEYS_ROOT_PATH}?{query_params}");
    let keys_body = get_request(&keys_path).await?;
    assert!(keys_body.is_object(), "Keys response must be an object");

    let posts_path = format!("{ROOT_PATH}?{query_params}");
    let posts_body = get_request(&posts_path).await?;
    assert!(posts_body.is_array(), "Posts response must be an array");

    let keys = keys_body["post_keys"]
        .as_array()
        .expect("Post key stream should expose a post_keys array");
    assert!(
        keys_body["last_post_score"].is_u64() || keys_body["last_post_score"].is_null(),
        "Post key stream should include an optional last_post_score"
    );
    let posts = posts_body
        .as_array()
        .expect("Post stream should be an array");

    assert_eq!(
        keys.len(),
        posts.len(),
        "Post key stream should contain the same number of entries as the post stream. {}",
        assertion_msg
    );

    verify_keys_match_posts(keys, posts);
    verify_last_post_score(&keys_body, posts);

    Ok(())
}

/// Verify that each key matches the format {author_id}:{post_id} from the corresponding post
fn verify_keys_match_posts(keys: &[Value], posts: &[Value]) {
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

        assert_eq!(
            key,
            format!("{author_id}:{post_id}"),
            "Post key should match the format {{author_id}}:{{post_id}}"
        );
    }
}

/// Verify that last_post_score is present
fn verify_last_post_score(keys_body: &Value, posts: &[Value]) {
    let last_post_score = keys_body["last_post_score"].as_u64();
    println!("last_post_score: {:?}", last_post_score);

    if posts.is_empty() {
        // If there are no posts, the score should be None
        assert!(
            last_post_score.is_none(),
            "last_post_score should be None when there are no posts"
        );
        return;
    }

    // If there are posts, the score should not be None
    last_post_score.expect("last_post_score should not be None when there are posts in the stream");
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_keys_align_with_posts() -> Result<()> {
    assert_post_keys_align_with_posts("sorting=timeline&limit=5", "").await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_keys_with_following_and_engagement_uses_graph() -> Result<()> {
    // Using following source with total_engagement sorting forces graph query
    let query = format!("observer_id={USER_ID}&source=following&sorting=total_engagement&limit=5");
    assert_post_keys_align_with_posts(
        &query,
        "when using graph query with following and engagement",
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_post_keys_with_multiple_tags_uses_graph() -> Result<()> {
    // Using multiple tags forces graph query
    let tags = format!("{TAG_LABEL_2},{TAG_LABEL_3}");
    let query = format!("tags={tags}&limit=5");
    assert_post_keys_align_with_posts(&query, "when using graph query with multiple tags").await
}
