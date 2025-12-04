use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_webapi::routes::v0::{
    endpoints::{SEARCH_USERS_BY_ID_ROUTE, SEARCH_USERS_BY_NAME_ROUTE},
    search::USER_ID_SEARCH_MIN_PREFIX_LEN,
};

fn format_search_users_by_name_prefix(prefix: &str) -> String {
    SEARCH_USERS_BY_NAME_ROUTE.replace("{prefix}", prefix)
}

fn format_search_users_by_id_prefix(prefix: &str) -> String {
    SEARCH_USERS_BY_ID_ROUTE.replace("{prefix}", prefix)
}

#[tokio_shared_rt::test(shared)]
async fn test_search_users_by_username() -> Result<()> {
    let user_prefix = "Jo";
    let url_path = format_search_users_by_name_prefix(user_prefix);
    let res = get_request(&url_path).await?;

    assert!(res.is_array());

    let users = res
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

#[tokio_shared_rt::test(shared)]
async fn test_search_users_by_id_empty_id() -> Result<()> {
    let url_path = format_search_users_by_id_prefix("");
    invalid_get_request(&url_path, StatusCode::NOT_FOUND).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_search_users_by_id_invalid_prefix_length() -> Result<()> {
    // Check if prefixes of at least 1 char, but lower than the min prefix limit are treated as invalid

    for i in 1..USER_ID_SEARCH_MIN_PREFIX_LEN {
        let invalid_prefix = "x".repeat(i).to_string();
        let url_path = format_search_users_by_id_prefix(&invalid_prefix);
        let res = invalid_get_request(&url_path, StatusCode::BAD_REQUEST).await?;

        assert!(res["error"].is_string(), "Error message should be a string");
        assert!(
            res["error"]
                .as_str()
                .unwrap_or("")
                .contains("ID prefix must be at least"),
            "Error message should mention min size limit"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_search_users_by_id() -> Result<()> {
    // Check endpoint results for an ID prefix with a valid length
    let id_prefix = "xte";
    let url_path = format_search_users_by_id_prefix(id_prefix);
    let res = get_request(&url_path).await?;

    assert!(res.is_array());

    let users = res
        .as_array()
        .expect("User search results should be an array");

    // Define the expected user IDs
    let expected_users = vec!["xtewe9x8yfuq5sr4tqrk5az47uz4qkt3gxaz5rms6nzugdfo8jao"];

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

#[tokio_shared_rt::test(shared)]
async fn test_search_non_existing_user() -> Result<()> {
    let non_existing_username = "idfjwfs8u9jfkoi"; // Username that doesn't exist
    let url_path = format_search_users_by_name_prefix(non_existing_username);

    let body = get_request(&url_path).await?;

    // Assert that it returns empty array
    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_search_non_existing_id() -> Result<()> {
    let non_existing_id = "abcdef"; // User ID that doesn't exist
    let url_path = format_search_users_by_id_prefix(non_existing_id);

    let body = get_request(&url_path).await?;

    // Assert that it returns empty array
    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_search_empty_username() -> Result<()> {
    let empty_username = ""; // Empty username
    let url_path = format_search_users_by_name_prefix(empty_username);

    // Since the username is part of the prefix, empty username appears as if
    // an unknown API endpoint is called, resulting in error 404 NOT_FOUND
    // (server sees "/search/user/by_name" instead of expected "/search/user/by_name/{prefix}")
    // Since it's thrown at router level, it has no message body
    invalid_get_request(&url_path, StatusCode::NOT_FOUND).await?;

    Ok(())
}
