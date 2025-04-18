use super::utils::find_post_details;
use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_reply_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let alice_keypair = Keypair::random();

    let alice = PubkyAppUser {
        bio: Some("test_homeserver_post_reply_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostReplyNotification:Alice".to_string(),
        status: None,
    };

    let alice_id = test.create_user(&alice_keypair, &alice).await?;

    let parent_post = PubkyAppPost {
        content: "Watcher:PostReplyNotification:Alice:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let alice_post_id = test.create_post(&alice_id, &parent_post).await?;

    let parent_uri = format!("pubky://{alice_id}/pub/pubky.app/posts/{alice_post_id}");

    let reply_post = PubkyAppPost {
        content: "Watcher:PostReplyNotification:Alice:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
        attachments: None,
    };

    let alice_reply_id = test.create_post(&alice_id, &reply_post).await?;

    // Verify that alice does not get a REPLY notification
    let notifications = Notification::get_by_id(&alice_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        0,
        "Alice cannot have any notification because she replies to its own post"
    );

    // Create new user to test the notication
    let bob_keypair = Keypair::random();

    let bob = PubkyAppUser {
        bio: Some("test_homeserver_post_reply_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostReplyNotification:Bob".to_string(),
        status: None,
    };

    let bob_id = test.create_user(&bob_keypair, &bob).await?;

    let reply_post = PubkyAppPost {
        content: "Watcher:PostReplyNotification:Bob:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
        attachments: None,
    };

    let bob_reply_id = test.create_post(&bob_id, &reply_post).await?;

    // Verify that alice gets a REPLY notification
    let notifications = Notification::get_by_id(&alice_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        1,
        "Alice should have at least one notification from bob reply"
    );

    let notification_body = &notifications[0].body;

    if let NotificationBody::Reply {
        replied_by,
        reply_uri,
        parent_post_uri,
    } = notification_body
    {
        assert_eq!(
            replied_by, &bob_id,
            "Reply Notification should contain the correct replier"
        );

        // CACHE_OP
        let reply_post_details = find_post_details(&bob_id, &bob_reply_id).await.unwrap();
        assert_eq!(
            reply_uri, &reply_post_details.uri,
            "Reply notification should contain correct reply URI"
        );

        let parent_post_details = find_post_details(&alice_id, &alice_post_id).await.unwrap();
        assert_eq!(
            parent_post_uri, &parent_post_details.uri,
            "Reply notification should contain correct parent URI"
        );
    } else {
        panic!("Expected a REPLY notification, found something else");
    }

    // DEL post.
    test.cleanup_post(&alice_id, &alice_reply_id).await?;
    test.cleanup_post(&bob_id, &bob_reply_id).await?;

    // Cleanup
    test.cleanup_user(&alice_id).await?;
    test.cleanup_post(&alice_id, &alice_post_id).await?;

    Ok(())
}
