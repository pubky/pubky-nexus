use crate::service::utils::HOST_URL;
use anyhow::Result;
use serde_json::json;

#[tokio::test]
async fn test_stream_most_followed() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Test retrieving the most followed users
    let res = client
        .do_get("/v0/stream/users?source=most_followed&limit=20")
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let most_followed_users = body.as_array().expect("User stream should be an array");

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
        assert!(exists, "Expected user ID not found: {}", id);
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
    let res = client
        .do_get("/v0/stream/users?source=most_followed&limit=5")
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let limited_users = body.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert_eq!(
        limited_users.len(),
        5,
        "Expected 5 users in the limited stream"
    );

    Ok(())
}

#[tokio::test]
async fn test_stream_pioneers() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Test retrieving the most followed users
    let res = client.do_get("/v0/stream/users?source=pioneers").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let pioneers_users = body.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert!(
        !pioneers_users.is_empty(),
        "There should be at least one user in the most followed stream"
    );

    // List of expected user IDs
    let expected_user_ids = vec![
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy",
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo",
        "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o",
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so",
    ];

    // Verify that each expected user ID is present in the response
    for id in &expected_user_ids {
        let exists = pioneers_users.iter().any(|f| f["details"]["id"] == *id);
        assert!(exists, "Expected user ID not found: {}", id);
    }

    // Additional checks for specific user attributes (e.g., name, follower counts)
    for user in pioneers_users {
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
    let res = client
        .do_get("/v0/stream/users?source=pioneers&limit=5")
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let limited_users = body.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert_eq!(
        limited_users.len(),
        5,
        "Expected 5 users in the limited stream"
    );

    Ok(())
}

#[tokio::test]
async fn test_stream_recommended() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // User ID to use for recommendations
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    // Test retrieving recommended users
    let res = client
        .do_get(&format!(
            "/v0/stream/users?source=recommended&user_id={}&limit=5",
            user_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let recommended_users = body.as_array().expect("User stream should be an array");

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
    let res = client
        .do_get(&format!(
            "/v0/stream/users?source=recommended&user_id={}&limit=3",
            user_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let limited_users = body.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert_eq!(
        limited_users.len(),
        3,
        "Expected 3 users in the limited stream"
    );

    Ok(())
}

#[tokio::test]
async fn test_stream_recommended_missing_user_id() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Test retrieving recommended users without providing user_id
    let res = client
        .do_get("/v0/stream/users?source=recommended&limit=5")
        .await?;

    // Assuming the endpoint returns 400 Bad Request
    assert_eq!(res.status(), 400, "Expected HTTP status 400 Bad Request");

    let body = res.json_body()?;
    assert!(
        body["error"]
            .as_str()
            .unwrap_or("")
            .contains("user_id query param must be provided"),
        "Error message should mention that user_id is required"
    );

    Ok(())
}

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

#[tokio::test]
async fn test_stream_users_by_username_search() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let username = "Jo";

    let res = client
        .do_get(&format!("/v0/stream/users/username?username={}", username))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let users = body
        .as_array()
        .expect("User search results should be an array");

    // Validate the response as needed
    assert!(!users.is_empty(), "User search should return results");

    Ok(())
}
