use crate::utils::recommended::DELETED;
use crate::utils::{get_request, invalid_get_request, server::TestServiceServer};
use anyhow::Result;
use axum::http::StatusCode;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::models::user::{UserStream, USER_MOSTFOLLOWED_KEY_PARTS};
use nexus_webapi::models::ErrorResponsePayload;
use serde_json::{json, Value};

// ##### MOST FOLLOWED USERS ######

const MOST_FOLLOWED_IDS: &str = "/v0/stream/users/ids?source=most_followed";

fn most_followed_key() -> String {
    format!("Sorted:{}", USER_MOSTFOLLOWED_KEY_PARTS.join(":"))
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_most_followed() -> Result<()> {
    // Test retrieving the most followed users
    let res = get_request("/v0/stream/users?source=most_followed&limit=20").await?;
    assert!(res.is_array());

    let most_followed_users = res.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert!(
        !most_followed_users.is_empty(),
        "There should be at least one user in the most followed stream"
    );

    // List of expected user IDs
    let expected_user_ids = vec![
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy",
        "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o",
        "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro",
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
    ];

    // Verify that each expected user ID is present in the response
    for id in &expected_user_ids {
        let exists = most_followed_users
            .iter()
            .any(|f| f["details"]["id"] == *id);
        assert!(exists, "Expected user ID not found: {id}");
    }

    // Additional checks for specific user attributes (e.g., name, follower counts)
    for user in most_followed_users {
        assert!(
            user["details"]["name"].is_string(),
            "Name should be a string"
        );
        assert!(user["details"]["bio"].is_string(), "Bio should be a string");
        assert!(
            user["counts"]["followers"].is_number(),
            "Follower counts should be a number"
        );
    }

    // Test limiting the results to 5 users
    let res = get_request("/v0/stream/users?source=most_followed&limit=5").await?;
    assert!(res.is_array());

    let limited_users = res.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert_eq!(
        limited_users.len(),
        5,
        "Expected 5 users in the limited stream"
    );

    Ok(())
}

/// Only a missing key is a miss: the read rebuilds the ranking from the graph and serves
/// the window it was asked for.
#[tokio_shared_rt::test(shared)]
async fn test_stream_most_followed_rebuilds_a_missing_ranking() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let key = most_followed_key();

    let _: () = redis_conn.del(&key).await?;
    let page = get_request(&format!("{MOST_FOLLOWED_IDS}&skip=5&limit=5")).await?;

    let window: Vec<String> = redis_conn.zrevrange(&key, 5, 9).await?;
    assert_eq!(window.len(), 5);
    assert_eq!(page, json!(window), "the page must be the requested window");

    Ok(())
}

/// An existing key whose window is empty is not a miss: a page past the end is served
/// empty, with no rebuild on the request path.
#[tokio_shared_rt::test(shared)]
async fn test_stream_most_followed_serves_an_empty_window_without_a_rebuild() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let key = most_followed_key();

    // Reading first rebuilds the ranking if it is missing, so the key exists.
    get_request(&format!("{MOST_FOLLOWED_IDS}&limit=1")).await?;
    let top: Vec<(String, f64)> = redis_conn.zrevrange_withscores(&key, 0, 0).await?;
    let (top_id, top_score) = top.first().cloned().expect("the fixture ranks users");
    // Follower counts are integers, so no rebuild can write this score.
    let marked = top_score + 0.5;
    let _: () = redis_conn.zadd(&key, &top_id, marked).await?;

    // Probed without `?`, so a failure cannot leave the marked score for the next reader.
    let probe = async {
        let page = get_request(&format!("{MOST_FOLLOWED_IDS}&skip=10000&limit=5")).await?;
        let after: Option<f64> = redis_conn.zscore(&key, &top_id).await?;
        anyhow::Ok((page, after))
    }
    .await;

    UserStream::refresh_most_followed().await?;
    let (page, after) = probe?;

    assert_eq!(page, json!([]), "a page past the end must be empty");
    assert_eq!(
        after,
        Some(marked),
        "an empty window must not rebuild the ranking"
    );

    Ok(())
}

/// A user deleted after the last refresh is still in the ranking. The read leaves them out
/// of the page and evicts them, and the next refresh does not bring them back.
#[tokio_shared_rt::test(shared)]
async fn test_stream_most_followed_filters_deleted_users() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let key = most_followed_key();
    let page_path = format!("{MOST_FOLLOWED_IDS}&limit=5");

    // Reading first rebuilds the ranking if it is missing, so the top score is the graph's.
    get_request(&page_path).await?;
    let top: Vec<(String, f64)> = redis_conn.zrevrange_withscores(&key, 0, 0).await?;
    let top_score = top.first().map(|(_, score)| *score).unwrap_or_default();
    let _: () = redis_conn.zadd(&key, DELETED, top_score + 1.0).await?;

    // Probed without `?`, so a failure cannot leave the planted member for the next reader.
    let probe = async {
        let first = get_request(&page_path).await?;
        let after_read: Option<f64> = redis_conn.zscore(&key, DELETED).await?;
        let second = get_request(&page_path).await?;
        anyhow::Ok((first, after_read, second))
    }
    .await;

    UserStream::refresh_most_followed().await?;
    let after_refresh: Option<f64> = redis_conn.zscore(&key, DELETED).await?;
    let (first, after_read, second) = probe?;

    let ids = |page: &Value| -> Vec<String> {
        page.as_array()
            .expect("User id stream should be an array")
            .iter()
            .map(|id| id.as_str().expect("ids are strings").to_string())
            .collect()
    };
    assert!(
        !ids(&first).contains(&DELETED.to_string()),
        "a deleted user must not be served"
    );
    assert_eq!(after_read, None, "the read must evict the deleted user");
    assert_eq!(
        ids(&second).len(),
        5,
        "the next read must serve a full page"
    );
    assert_eq!(
        after_refresh, None,
        "a refresh must leave deleted users out"
    );

    Ok(())
}

// ##### RECOMMENDED USERS ######

#[tokio_shared_rt::test(shared)]
async fn test_stream_recommended() -> Result<()> {
    // User ID to use for recommendations
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    // Test retrieving recommended users
    let res = get_request(&format!(
        "/v0/stream/users?source=recommended&user_id={user_id}&limit=5"
    ))
    .await?;

    assert!(res.is_array());

    let recommended_users = res.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert!(
        !recommended_users.is_empty(),
        "There should be at least one user in the recommended stream"
    );

    // Additional checks for specific user attributes
    for user in recommended_users {
        assert!(
            user["details"]["name"].is_string(),
            "Name should be a string"
        );
        assert!(user["details"]["bio"].is_string(), "Bio should be a string");
        assert!(
            user["counts"]["followers"].is_number(),
            "Follower counts should be a number"
        );

        // Since these are recommended users, we should verify that the viewer is not already following them
        let relationship = &user["relationship"];
        assert!(
            relationship["following"].as_bool() == Some(false),
            "Viewer should not be following the recommended user"
        );
    }

    // Test limiting the results to 3 users
    let res = get_request(&format!(
        "/v0/stream/users?source=recommended&user_id={user_id}&limit=3"
    ))
    .await?;

    assert!(res.is_array());

    let limited_users = res.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert_eq!(
        limited_users.len(),
        3,
        "Expected 3 users in the limited stream"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_recommended_missing_user_id() -> Result<()> {
    // Test retrieving recommended users without providing user_id
    let res = invalid_get_request(
        "/v0/stream/users?source=recommended&limit=5",
        StatusCode::BAD_REQUEST,
    )
    .await?;

    let error_response: ErrorResponsePayload =
        serde_json::from_value(res).expect("Response should be a valid ErrroResponsePayload");
    assert!(
        error_response
            .error
            .contains("user_id query param must be provided"),
        "Error message should mention that user_id is required, got: {}",
        error_response.error
    );

    Ok(())
}
