use super::utils::find_post_details;
use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::notification::{Notification, NotificationBody};
use pubky_nexus::models::pubky_app::{PostKind, PubkyAppPost, PubkyAppUser};

#[tokio::test]
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
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let alice_post_id = test.create_post(&alice_id, &parent_post).await?;

    let parent_uri = format!("pubky://{alice_id}/pub/pubky.app/posts/{alice_post_id}");

    let reply_post = PubkyAppPost {
        content: "Watcher:PostReplyNotification:Alice:Reply".to_string(),
        kind: PostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
    };

    let alice_reply_id = test.create_post(&alice_id, &reply_post).await?;

    // Verify that alice does not get a REPLY notification
    let notifications = Notification::get_by_id(&alice_id, None, None, None, None)
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
        kind: PostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
    };

    let bob_reply_id = test.create_post(&bob_id, &reply_post).await?;

    // Verify that alice gets a REPLY notification
    let notifications = Notification::get_by_id(&alice_id, None, None, None, None)
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

    // // TODO: Impl DEL post. Assert the reply does not exist in Nexus
    test.cleanup_post(&alice_id, &alice_reply_id).await?;
    test.cleanup_post(&bob_id, &bob_reply_id).await?;
    // let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap();

    // assert!(result_post.is_none(), "The post should have been deleted");

    // After deletion, fetch the post thread again and confirm the reply is gone
    // let thread_after_deletion = PostThread::get_by_id(&user_id, &parent_id, None, 0, 10)
    //     .await
    //     .expect("Failed to fetch post thread after deletion")
    //     .expect("The post thread should exist after deletion");

    // Cleanup
    test.cleanup_user(&alice_id).await?;
    test.cleanup_post(&alice_id, &alice_post_id).await?;

    Ok(())
}