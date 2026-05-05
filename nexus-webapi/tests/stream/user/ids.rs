use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::Value;

const IDS_ROOT_PATH: &str = "/v0/stream/users/ids";
const USERS_ROOT_PATH: &str = "/v0/stream/users";
const USER_ID: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

async fn assert_user_ids_align(query_params: &str, assertion_msg: &str) -> Result<()> {
    let ids_path = if query_params.is_empty() {
        IDS_ROOT_PATH.to_string()
    } else {
        format!("{IDS_ROOT_PATH}?{query_params}")
    };
    let ids_body = get_request(&ids_path).await?;
    assert!(
        ids_body.is_array(),
        "User id stream response must be an array"
    );

    let users_path = if query_params.is_empty() {
        USERS_ROOT_PATH.to_string()
    } else {
        format!("{USERS_ROOT_PATH}?{query_params}")
    };
    let users_body = get_request(&users_path).await?;
    assert!(
        users_body.is_array(),
        "User stream response must be an array"
    );

    let id_entries = ids_body
        .as_array()
        .expect("User id stream should be an array of IDs");
    let users = users_body
        .as_array()
        .expect("User stream should provide an array of user objects");

    assert_eq!(
        id_entries.len(),
        users.len(),
        "User id stream should return the same number of entries as the detailed stream. {}",
        assertion_msg
    );

    verify_ids_match_users(id_entries, users);

    Ok(())
}

fn verify_ids_match_users(ids: &[Value], users: &[Value]) {
    for (id_value, user_value) in ids.iter().zip(users.iter()) {
        let id = id_value
            .as_str()
            .expect("User id entries should be string values");
        let user_id = user_value["details"]["id"]
            .as_str()
            .expect("User stream entries should include an id");

        assert_eq!(
            id, user_id,
            "User id entries must match the detailed stream ids"
        );
    }
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_user_ids_most_followed_align() -> Result<()> {
    assert_user_ids_align(
        "source=most_followed&limit=5",
        "when retrieving the most followed stream",
    )
    .await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_user_ids_followers_align() -> Result<()> {
    let query = format!("source=followers&user_id={USER_ID}&limit=5");
    assert_user_ids_align(&query, "for follower streams requiring a user id").await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_user_ids_influencers_wot_1_align() -> Result<()> {
    let query =
        format!("source=influencers&user_id={USER_ID}&timeframe=this_month&reach=wot_1&limit=5");
    assert_user_ids_align(&query, "for influencers stream with reach wot_1").await
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_user_ids_influencers_wot_5_invalid() -> Result<()> {
    let endpoint = format!(
        "{IDS_ROOT_PATH}?source=influencers&user_id={USER_ID}&timeframe=this_month&reach=wot_5&limit=5"
    );
    invalid_get_request(&endpoint, StatusCode::BAD_REQUEST).await?;
    Ok(())
}
