use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};
use pubky_nexus::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};

#[tokio_shared_rt::test(shared)]
async fn test_delete_parent_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create User A who makes the original post
    let keypair_a = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyParentDeleteNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&keypair_a, &user_a).await?;

    // Create User B who replies to User A's post
    let keypair_b = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyParentDeleteNotification:UserB".to_string(),
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

    // User B replies to User A's post
    let reply = PubkyAppPost {
        content: "Reply by User B".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(format!(
            "pubky://{}/pub/pubky.app/posts/{}",
            user_a_id, post_id
        )),
        embed: None,
        attachments: None,
    };
    let reply_id = test.create_post(&user_b_id, &reply).await?;

    // User A deletes their original post
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
            linked_uri,
            &format!("pubky://{}/pub/pubky.app/posts/{}", user_b_id, reply_id),
            "Notification should contain the correct reply URI"
        );
        assert_eq!(
            delete_source,
            &PostChangedSource::ReplyParent,
            "Delete notification should have the correct source"
        );
    } else {
        panic!("Expected a PostDeleted notification, found something else");
    }

    Ok(())
}
