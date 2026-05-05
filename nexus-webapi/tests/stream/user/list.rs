use crate::utils::{invalid_post_request, post_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_webapi::models::ErrorResponse;
use serde_json::json;
// ##### LIST OF USERS BY ID ######

#[tokio_shared_rt::test(shared)]
async fn test_stream_users_by_ids_valid_request() -> Result<()> {
    // List of valid user IDs
    let user_ids = vec![
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
        "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo",
        "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy",
    ];

    // Prepare the request body
    let request_body = json!({
        "user_ids": user_ids,
        "viewer_id": null
    });

    // Send the POST request to the endpoint
    let res = post_request("/v0/stream/users/by_ids", request_body).await?;

    assert!(res.is_array(), "Response body should be an array");

    let users = res.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert_eq!(users.len(), 3, "Expected 3 users in the response");

    // Verify that each expected user ID is present in the response
    for id in &user_ids {
        let exists = users.iter().any(|u| u["details"]["id"] == *id);
        assert!(exists, "Expected user ID not found: {id}");
    }

    // Additional checks for specific user attributes
    for user in users {
        assert!(
            user["details"]["name"].is_string(),
            "Name should be a string"
        );
        assert!(
            user["counts"]["followers"].is_number(),
            "Follower counts should be a number"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_users_by_ids_limit_exceeded() -> Result<()> {
    // Generate 101 invalid (short) user IDs to exceed the max limit of 100.
    // The length check should fail-fast BEFORE element validation, so even though
    // these IDs are not valid 52-char PubkyIds, the length error comes first.
    let user_ids: Vec<String> = (0..101).map(|i| format!("user_id_{i}")).collect();

    let request_body = json!({
        "user_ids": user_ids,
        "viewer_id": null
    });

    // Send the POST request to the endpoint
    let res = invalid_post_request(
        "/v0/stream/users/by_ids",
        request_body,
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // Verify the error message mentions the length limit (fail-fast), not element validation
    let error_response: ErrorResponse =
        serde_json::from_value(res).expect("Response should be a valid ErrorResponse");
    assert!(
        error_response.error.contains("100")
            || error_response.error.to_lowercase().contains("maximum"),
        "Error message should mention the maximum items limit, got: {}",
        error_response.error
    );

    Ok(())
}

// todo: this will no longer pass - id validation will fail
// #[tokio_shared_rt::test(shared)]
async fn test_stream_users_by_ids_with_invalid_ids() -> Result<()> {
    // Valid and invalid user IDs
    let user_ids = vec![
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro", // Valid
        "nonexistent_user_id",                                  // Invalid
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo", // Valid
    ];

    let request_body = json!({
        "user_ids": user_ids,
        "viewer_id": null
    });

    let res = post_request("/v0/stream/users/by_ids", request_body).await?;

    assert!(res.is_array(), "Response body should be an array");

    let users = res.as_array().expect("User stream should be an array");

    // Expected valid user IDs
    let expected_user_ids = vec![
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo",
    ];

    // Check that only valid users are returned
    assert_eq!(
        users.len(),
        expected_user_ids.len(),
        "Expected {} users in the response",
        expected_user_ids.len()
    );

    for id in &expected_user_ids {
        let exists = users.iter().any(|u| u["details"]["id"] == *id);
        assert!(exists, "Expected user ID not found: {id}");
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_users_by_ids_empty_list() -> Result<()> {
    // Empty list of user IDs
    let user_ids: Vec<String> = Vec::new();

    let request_body = json!({
        "user_ids": user_ids,
        "viewer_id": null
    });

    let res = invalid_post_request(
        "/v0/stream/users/by_ids",
        request_body,
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // Deserialize response into ErrorResponse
    let error_response: ErrorResponse =
        serde_json::from_value(res).expect("Response should be a valid ErrorResponse");
    assert!(
        error_response
            .error
            .contains("user_ids: At least 1 item(s) required"),
        "Error field should mention that user_ids cannot be empty, got: {}",
        error_response.error
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_users_by_ids_with_viewer_id() -> Result<()> {
    // List of valid user IDs
    let user_ids = vec![
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo",
    ];

    let viewer_id = "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o";

    let request_body = json!({
        "user_ids": user_ids,
        "viewer_id": viewer_id
    });

    let res = post_request("/v0/stream/users/by_ids", request_body).await?;

    assert!(res.is_array(), "Response body should be an array");

    let users = res.as_array().expect("User stream should be an array");

    // Check that the correct number of users is returned
    assert_eq!(
        users.len(),
        user_ids.len(),
        "Expected {} users in the response",
        user_ids.len()
    );

    // Check that viewer_id relationships are properly included
    for user in users {
        let relationship = &user["relationship"];
        // Verify that relationship fields are present and correctly formatted
        assert!(
            relationship["followed_by"].is_boolean(),
            "is_follower should be a boolean"
        );
        assert!(
            relationship["following"].is_boolean(),
            "is_following should be a boolean"
        );
    }

    Ok(())
}
