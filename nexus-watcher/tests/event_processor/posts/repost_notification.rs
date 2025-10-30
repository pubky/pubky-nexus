use super::utils::find_post_details;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_repost_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let alice_kp = Keypair::random();

    let alice = PubkyAppUser {
        bio: Some("test_homeserver_post_repost_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostRepostNotification:Alice".to_string(),
        status: None,
    };

    let alice_id = test.create_user(&alice_kp, &alice).await?;

    let parent_post = PubkyAppPost {
        content: "Watcher:PostRepostNotification:Alice:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let alice_post_id = test.create_post(&alice_kp, &parent_post).await?;

    let parent_absolute_uri = post_uri_builder(alice_id.clone(), alice_post_id.clone());

    let alice_repost = PubkyAppPost {
        content: "Watcher:PostRepostNotification:Alice:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: parent_absolute_uri.clone(),
        }),
        attachments: None,
    };

    let alice_reply_id = test.create_post(&alice_kp, &alice_repost).await?;

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
    let bob_kp = Keypair::random();

    let bob = PubkyAppUser {
        bio: Some("test_homeserver_post_repost_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostRepostNotification:Bob".to_string(),
        status: None,
    };

    let bob_id = test.create_user(&bob_kp, &bob).await?;

    let bob_repost = PubkyAppPost {
        content: "Watcher:PostRepostNotification:Bob:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: parent_absolute_uri.clone(),
        }),
        attachments: None,
    };

    let bob_reply_id = test.create_post(&bob_kp, &bob_repost).await?;

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
    test.cleanup_post(&alice_kp, &alice_reply_id).await?;
    test.cleanup_post(&bob_kp, &bob_reply_id).await?;

    // Cleanup
    test.cleanup_user(&alice_kp).await?;
    test.cleanup_post(&alice_kp, &alice_post_id).await?;

    Ok(())
}
