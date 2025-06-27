use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;

// ##### MOST FOLLOWED USERS ######

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

    assert!(
        res["error"]
            .as_str()
            .unwrap_or("")
            .contains("user_id query param must be provided"),
        "Error message should mention that user_id is required"
    );

    Ok(())
}
