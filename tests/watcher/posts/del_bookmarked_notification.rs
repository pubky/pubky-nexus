use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_app_specs::{
    traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppPostKind, PubkyAppUser,
};
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};

#[tokio::test]
async fn test_delete_bookmarked_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create User A who makes the original post
    let keypair_a = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:BookmarkedPostDeleteNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&keypair_a, &user_a).await?;

    // Create User B who bookmarks User A's post
    let keypair_b = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:BookmarkedPostDeleteNotification:UserB".to_string(),
        status: None,
    };
    let user_b_id = test.create_user(&keypair_b, &user_b).await?;

    // User A creates a post
    let post = PubkyAppPost {
        content: "Original post by User A".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_a_id, &post).await?;

    // User B bookmarks User A's post
    let bookmark = PubkyAppBookmark {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_a_id, post_id),
        created_at: 0,
    };
    let bookmark_blob = serde_json::to_vec(&bookmark)?;
    let bookmark_url = format!(
        "pubky://{}/pub/pubky.app/bookmarks/{}",
        user_b_id,
        bookmark.create_id()
    );
    test.create_bookmark(&bookmark_url, bookmark_blob).await?;

    // User A deletes their post
    test.cleanup_post(&user_a_id, &post_id).await?;

    // Verify that User B receives a notification about the deletion
    let notifications = Notification::get_by_id(&user_b_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        1,
        "User B should have exactly one notification"
    );

    if let NotificationBody::PostDeleted {
        delete_source,
        deleted_by,
        deleted_uri,
        linked_uri,
    } = &notifications[0].body
    {
        assert_eq!(
            deleted_by, &user_a_id,
            "Notification should specify the correct user who deleted the post"
        );
        assert_eq!(
            deleted_uri,
            &format!("pubky://{}/pub/pubky.app/posts/{}", user_a_id, post_id),
            "Notification should contain the correct deleted post URI"
        );
        assert_eq!(
            linked_uri, &bookmark_url,
            "Notification should contain the correct bookmark URI"
        );
        assert_eq!(
            delete_source,
            &PostChangedSource::Bookmark,
            "Delete notification should have the correct source"
        );
    } else {
        panic!("Expected a PostDeleted notification, found something else");
    }

    Ok(())
}
