use crate::service::utils::HOST_URL;
use anyhow::Result;

#[tokio_shared_rt::test(shared)]
async fn test_stream_following() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!(
            "/v0/stream/users?user_id={}&source=following&limit=20",
            user_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let following = body.as_array().expect("User stream should be an array");

    // Check if the user is following the expected number of users
    assert_eq!(following.len(), 15, "Unexpected number of users followed");

    // List of expected following IDs
    let expected_following_ids = vec![
        "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy",
        "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y",
        // Add more expected following IDs
    ];

    // Verify that each expected following ID is present in the response
    for id in &expected_following_ids {
        let exists = following.iter().any(|f| f["details"]["id"] == *id);
        assert!(exists, "Expected following ID not found: {}", id);
    }

    // Additional checks for specific user attributes (e.g., name, bio)
    for follow in following {
        assert!(
            follow["details"]["name"].is_string(),
            "Name should be a string"
        );
        assert!(
            follow["details"]["bio"].is_string(),
            "Bio should be a string"
        );
        assert!(
            follow["counts"]["followers"].is_number(),
            "Follower counts should be a number"
        );
    }

    // Test non-existing user
    let res = client
        .do_get(&format!(
            "/v0/stream/users?user_id={}&source=following",
            "bad_user_id"
        ))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}
