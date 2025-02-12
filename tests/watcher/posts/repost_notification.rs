use super::utils::find_post_details;
use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser};
use pubky_nexus::models::notification::{Notification, NotificationBody};
use pubky_nexus::types::Pagination;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_repost_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let alice_keypair = Keypair::random();

    let alice = PubkyAppUser {
        bio: Some("test_homeserver_post_repost_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostRepostNotification:Alice".to_string(),
        status: None,
    };

    let alice_id = test.create_user(&alice_keypair, &alice).await?;

    let parent_post = PubkyAppPost {
        content: "Watcher:PostRepostNotification:Alice:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let alice_post_id = test.create_post(&alice_id, &parent_post).await?;

    let parent_uri = format!("pubky://{alice_id}/pub/pubky.app/posts/{alice_post_id}");

    let alice_repost = PubkyAppPost {
        content: "Watcher:PostRepostNotification:Alice:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: parent_uri.clone(),
        }),
        attachments: None,
    };

    let alice_reply_id = test.create_post(&alice_id, &alice_repost).await?;

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
        bio: Some("test_homeserver_post_repost_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostRepostNotification:Bob".to_string(),
        status: None,
    };

    let bob_id = test.create_user(&bob_keypair, &bob).await?;

    let bob_repost = PubkyAppPost {
        content: "Watcher:PostRepostNotification:Bob:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: parent_uri.clone(),
        }),
        attachments: None,
    };

    let bob_reply_id = test.create_post(&bob_id, &bob_repost).await?;

    // Verify that alice gets a REPLY notification
    let notifications = Notification::get_by_id(&alice_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        1,
        "Alice should have at least one notification from bob repost"
    );

    let notification_body = &notifications[0].body;

    if let NotificationBody::Repost {
        reposted_by,
        embed_uri,
        repost_uri,
    } = notification_body
    {
        assert_eq!(
            reposted_by, &bob_id,
            "Respost Notification should contain the correct replier"
        );

        // CACHE_OP
        let bob_repost_details = find_post_details(&bob_id, &bob_reply_id).await.unwrap();
        assert_eq!(
            repost_uri, &bob_repost_details.uri,
            "Repost notification should contain correct reply URI"
        );

        let parent_post_details = find_post_details(&alice_id, &alice_post_id).await.unwrap();
        assert_eq!(
            embed_uri, &parent_post_details.uri,
            "Repost notification should contain correct parent URI"
        );
    } else {
        panic!("Expected a REPOST notification, found something else");
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
