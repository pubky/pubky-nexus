use crate::utils::get_request;
use anyhow::Result;

#[tokio_shared_rt::test(shared)]
async fn test_stream_following() -> Result<()> {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = get_request(&format!(
        "/v0/stream/users?user_id={user_id}&source=following&limit=20"
    ))
    .await?;
    assert!(res.is_array());

    let following = res.as_array().expect("User stream should be an array");

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
        assert!(exists, "Expected following ID not found: {id}");
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

    // Test non-existing user - should return empty stream
    let body = get_request(&format!(
        "/v0/stream/users?user_id={}&source=following",
        "bad_user_id"
    ))
    .await?;

    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

    Ok(())
}
