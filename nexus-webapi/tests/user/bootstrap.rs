use std::collections::HashSet;

use crate::utils::get_request;
use anyhow::Result;
use nexus_common::models::bootstrap::Bootstrap;
use nexus_common::models::notification::Notification;
use nexus_common::models::notification::NotificationBody;

#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_user() -> Result<()> {
    let user_id = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy";
    let follower_id = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

    // Init TestService, incl. DBs, before using redis connection
    crate::utils::server::TestServiceServer::get_test_server().await;

    // Create test notifications for the user
    Notification::new_follow(follower_id, user_id, false)
        .await
        .expect("Failed to create follow notification");

    let body = get_request(&format!("/v0/bootstrap/{user_id}")).await?;
    let user_bootstrap_respose: Bootstrap = serde_json::from_value(body).unwrap();

    // Assert the user is indexed
    assert!(user_bootstrap_respose.indexed, "User should be indexed");

    // Assert the lists
    assert_eq!(user_bootstrap_respose.ids.stream.len(), 20);
    assert_eq!(user_bootstrap_respose.ids.influencers.len(), 3);
    assert_eq!(user_bootstrap_respose.ids.recommended.len(), 5);
    assert!(user_bootstrap_respose.ids.hot_tags.len() <= 5);

    let user_ids: HashSet<String> = user_bootstrap_respose
        .users
        .0
        .iter()
        .map(|user_view| user_view.details.id.to_string())
        .collect();

    // Assert post authors are included in the users list
    for post in &user_bootstrap_respose.posts.0 {
        let author_id = &post.details.author;
        assert!(
            user_ids.contains(author_id),
            "user_ids is missing author `{author_id}`"
        );
    }

    // MLOW1TGL5BKH4 has 2 attachments pointing to Cairo's files
    assert!(
        !user_bootstrap_respose.files.is_empty(),
        "Bootstrap should contain file metadata for post attachments"
    );
    assert_eq!(
        user_bootstrap_respose.files.len(),
        2,
        "Expected 2 file details from MLOW1TGL5BKH4 attachments"
    );
    let file_ids: HashSet<String> = user_bootstrap_respose
        .files
        .iter()
        .map(|f| f.id.clone())
        .collect();
    assert!(file_ids.contains("2ZK3A1B2C3D40"));
    assert!(file_ids.contains("2ZK3E5F6G7H80"));

    // Assert notifications count for indexed user
    assert_eq!(
        user_bootstrap_respose.notifications.len(),
        1,
        "Expected 1 follow notification"
    );

    // Verify the notification is a follow notification
    if let Some(notification) = user_bootstrap_respose.notifications.first() {
        match &notification.body {
            NotificationBody::Follow { followed_by } => {
                assert_eq!(
                    followed_by, follower_id,
                    "Follow notification should be from the correct user"
                );
            }
            _ => panic!("Expected a Follow notification"),
        }
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_user_not_indexed() -> Result<()> {
    // Use a random pubky ID that doesn't exist in the system
    let user_id = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhi";

    let body = get_request(&format!("/v0/bootstrap/{user_id}")).await?;
    let user_bootstrap_response: Bootstrap = serde_json::from_value(body).unwrap();

    // Assert the user is not indexed
    assert!(
        !user_bootstrap_response.indexed,
        "User should not be indexed"
    );

    // Even though user is not indexed, we should still get some data
    // (influencers, hot_tags, etc.) but no recommended users
    assert_eq!(
        user_bootstrap_response.ids.recommended.len(),
        0,
        "Non-indexed users should not have recommended users"
    );
    // Influencers and hot_tags should still be populated (global data)
    assert!(user_bootstrap_response.ids.influencers.len() <= 3);
    assert!(user_bootstrap_response.ids.hot_tags.len() <= 40);

    // Notifications should be empty for non-indexed users
    assert_eq!(
        user_bootstrap_response.notifications.len(),
        0,
        "Non-indexed users should not have notifications"
    );

    // Files count should match the unique attachment URIs found in the post stream
    let expected_uris: HashSet<String> = user_bootstrap_response
        .posts
        .0
        .iter()
        .filter_map(|p| p.details.attachments.as_ref())
        .flatten()
        .cloned()
        .collect();
    assert_eq!(
        user_bootstrap_response.files.len(),
        expected_uris.len(),
        "Files count should match the number of unique attachment URIs in posts"
    );

    Ok(())
}
