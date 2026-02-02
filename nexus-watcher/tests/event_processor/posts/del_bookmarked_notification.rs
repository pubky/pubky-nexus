use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{
    bookmark_uri_builder, post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPost,
    PubkyAppPostKind, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_delete_bookmarked_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create User A who makes the original post
    let user_a_kp = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:BookmarkedPostDeleteNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&user_a_kp, &user_a).await?;

    // Create User B who bookmarks User A's post
    let user_b_kp = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:BookmarkedPostDeleteNotification:UserB".to_string(),
        status: None,
    };
    let user_b_id = test.create_user(&user_b_kp, &user_b).await?;

    // User A creates a post
    let post = PubkyAppPost {
        content: "Original post by User A".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&user_a_kp, &post).await?;

    // User B bookmarks User A's post
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(user_a_id.clone(), post_id.clone()),
        created_at: 0,
    };
    let bookmark_absolute_url = bookmark_uri_builder(user_b_id.clone(), bookmark.create_id());
    let bookmark_path = bookmark.hs_path();
    test.put(&user_b_kp, &bookmark_path, bookmark).await?;

    // User A deletes their post
    test.cleanup_post(&user_a_kp, &post_path).await?;

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
            &format!("pubky://{user_a_id}/pub/pubky.app/posts/{post_id}"),
            "Notification should contain the correct deleted post URI"
        );
        assert_eq!(
            linked_uri, &bookmark_absolute_url,
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
