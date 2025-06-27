use crate::utils::get_request;
use anyhow::Result;

#[tokio_shared_rt::test(shared)]
async fn test_stream_users_by_username_search() -> Result<()> {
    let username = "Jo";

    let res = get_request(&format!("/v0/stream/users/username?username={username}")).await?;
    assert!(res.is_array());

    let users = res
        .as_array()
        .expect("User search results should be an array");

    // Validate the response as needed
    assert!(!users.is_empty(), "User search should return results");

    Ok(())
}
