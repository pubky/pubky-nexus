use anyhow::Result;

use pubky_nexus::{
    models::{
        post::{PostCounts, PostStream, POST_PER_USER_KEY_PARTS, POST_TIMELINE_KEY_PARTS},
        user::{UserCounts, UserStream, USER_PIONEERS_KEY_PARTS},
    },
    RedisOps,
};

pub async fn find_post_counts(post_key: &[&str]) -> PostCounts {
    PostCounts::try_from_index_json(&post_key)
        .await
        .unwrap()
        .expect("The post count was not served from Nexus cache")
}

pub async fn check_member_global_timeline_user_post(
    user_id: &str,
    post_id: &str,
) -> Result<Option<isize>> {
    let post_key: &[&str] = &[&user_id, &post_id];
    let global_timeline = PostStream::check_sorted_set_member(&POST_TIMELINE_KEY_PARTS, post_key)
        .await
        .unwrap();
    Ok(global_timeline)
}

pub async fn check_member_user_post_timeline(
    user_id: &str,
    post_id: &str,
) -> Result<Option<isize>> {
    let post_stream_key_parts = [&POST_PER_USER_KEY_PARTS[..], &[&user_id]].concat();
    let post_timeline = PostStream::check_sorted_set_member(&post_stream_key_parts, &[&post_id])
        .await
        .unwrap();
    Ok(post_timeline)
}

pub async fn check_member_user_pioneer(user_id: &str) -> Result<Option<isize>> {
    let pioneer_score = UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&user_id])
        .await
        .unwrap();
    Ok(pioneer_score)
}

pub async fn find_user_counts(user_id: &str) -> UserCounts {
    UserCounts::try_from_index_json(&[&user_id])
        .await
        .unwrap()
        .expect("User count not found with that ID")
}
