use crate::service::utils::HOST_URL;
use anyhow::Result;
use serde_json::json;

// ##### LIST OF USERS BY ID ######

#[tokio::test]
async fn test_stream_users_by_ids_valid_request() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

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
    let res = client
        .do_post("/v0/stream/users/by_ids", request_body)
        .await?;

    assert_eq!(res.status(), 200, "Expected HTTP status 200 OK");

    let body = res.json_body()?;
    assert!(body.is_array(), "Response body should be an array");

    let users = body.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert_eq!(users.len(), 3, "Expected 3 users in the response");

    // Verify that each expected user ID is present in the response
    for id in &user_ids {
        let exists = users.iter().any(|u| u["details"]["id"] == *id);
        assert!(exists, "Expected user ID not found: {}", id);
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

#[tokio::test]
async fn test_stream_users_by_ids_limit_exceeded() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Generate a list of 1001 user IDs to exceed the limit
    let mut user_ids = Vec::with_capacity(1001);
    for i in 0..1001 {
        user_ids.push(format!("user_id_{}", i));
    }

    let request_body = json!({
        "user_ids": user_ids,
        "viewer_id": null
    });

    // Send the POST request to the endpoint
    let res = client
        .do_post("/v0/stream/users/by_ids", request_body)
        .await?;

    // Expecting a 400 Bad Request due to exceeding the limit
    assert_eq!(res.status(), 400, "Expected HTTP status 400 Bad Request");

    Ok(())
}

#[tokio::test]
async fn test_stream_users_by_ids_with_invalid_ids() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

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

    let res = client
        .do_post("/v0/stream/users/by_ids", request_body)
        .await?;

    // Assuming the endpoint returns 200 OK with valid users only
    assert_eq!(res.status(), 200, "Expected HTTP status 200 OK");

    let body = res.json_body()?;
    assert!(body.is_array(), "Response body should be an array");

    let users = body.as_array().expect("User stream should be an array");

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
        assert!(exists, "Expected user ID not found: {}", id);
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_users_by_ids_empty_list() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Empty list of user IDs
    let user_ids: Vec<String> = Vec::new();

    let request_body = json!({
        "user_ids": user_ids,
        "viewer_id": null
    });

    let res = client
        .do_post("/v0/stream/users/by_ids", request_body)
        .await?;

    // Expecting a 400 Bad Request due to empty user_ids list
    assert_eq!(res.status(), 400, "Expected HTTP status 400 Bad Request");

    let body = res.json_body()?;
    assert!(
        body["error"].as_str().unwrap_or("").contains("empty"),
        "Error message should mention that user_ids cannot be empty"
    );

    Ok(())
}

#[tokio::test]
async fn test_stream_users_by_ids_with_viewer_id() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

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

    let res = client
        .do_post("/v0/stream/users/by_ids", request_body)
        .await?;

    assert_eq!(res.status(), 200, "Expected HTTP status 200 OK");

    let body = res.json_body()?;
    assert!(body.is_array(), "Response body should be an array");

    let users = body.as_array().expect("User stream should be an array");

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
