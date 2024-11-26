use anyhow::Result;

const HOST_URL: &str = "http://localhost:8080";

#[tokio::test]
async fn test_search_users_by_username() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let username = "Jo";

    let res = client
        .do_get(&format!("/v0/search/users?username={}", username))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let users = body
        .as_array()
        .expect("User search results should be an array");

    // Define the expected user IDs
    let expected_users = vec![
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo",
    ];

    // Convert the actual result to a Vec of strings
    let actual_users: Vec<String> = users
        .iter()
        .map(|user| {
            user.as_str()
                .expect("User ID should be a string")
                .to_string()
        })
        .collect();

    // Assert that the actual result matches the expected result
    assert_eq!(actual_users, expected_users);

    Ok(())
}

#[tokio::test]
async fn test_search_non_existing_user() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let non_existing_username = "idfjwfs8u9jfkoi"; // Username that doesn't exist

    let res = client
        .do_get(&format!(
            "/v0/search/users?username={}",
            non_existing_username
        ))
        .await?;

    // Assert that the status code is 404 Not Found
    assert_eq!(res.status(), 404);

    let body = res.json_body()?;
    assert!(
        body["error"].is_string(),
        "Error message should be a string"
    );

    // Optional: Check that the error message contains the correct details
    assert!(
        body["error"]
            .as_str()
            .unwrap_or("")
            .contains(non_existing_username),
        "Error message should mention the non-existing username"
    );

    Ok(())
}

#[tokio::test]
async fn test_search_empty_username() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let empty_username = ""; // Empty username

    let res = client
        .do_get(&format!("/v0/search/users?username={}", empty_username))
        .await?;

    // Assert that the status code is 400 Bad Request
    assert_eq!(res.status(), 400);

    let body = res.json_body()?;
    assert!(
        body["error"].is_string(),
        "Error message should be a string"
    );

    // Optional: Check that the error message contains the correct details
    assert!(
        body["error"]
            .as_str()
            .unwrap_or("")
            .contains("Username cannot be empty"),
        "Error message should mention that the username cannot be empty"
    );

    Ok(())
}