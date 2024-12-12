use crate::service::utils::HOST_URL;
use anyhow::Result;

#[tokio_shared_rt::test(shared)]
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
