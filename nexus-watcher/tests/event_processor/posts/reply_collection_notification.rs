use super::utils::collection_post;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::notification::{Notification, NotificationBody, PostChangedSource};
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_reply_to_collection_notification() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let alice_kp = Keypair::random();
    let alice = PubkyAppUser {
        bio: Some("test_reply_to_collection_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyToCollectionNotification:Alice".to_string(),
        status: None,
    };
    let alice_id = test.create_user(&alice_kp, &alice).await?;

    // Alice creates a collection
    let collection = collection_post("discussed");
    let (alice_post_id, alice_post_path) = test.create_post(&alice_kp, &collection).await?;
    let parent_absolute_uri = post_uri_builder(alice_id.clone(), alice_post_id.clone());

    let bob_kp = Keypair::random();
    let bob = PubkyAppUser {
        bio: Some("test_reply_to_collection_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyToCollectionNotification:Bob".to_string(),
        status: None,
    };
    let bob_id = test.create_user(&bob_kp, &bob).await?;

    // Bob replies to Alice's collection
    let reply_post = PubkyAppPost {
        content: "Watcher:ReplyToCollectionNotification:Bob:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_absolute_uri.clone()),
        embed: None,
        attachments: None,
        lock: None,
    };
    let (bob_reply_id, bob_reply_path) = test.create_post(&bob_kp, &reply_post).await?;

    // Verify that Alice gets a collection-specific reply notification
    let notifications = Notification::get_by_id(&alice_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        1,
        "Alice should have exactly one notification from Bob's reply"
    );

    if let NotificationBody::Reply {
        replied_by,
        reply_uri,
        parent_post_uri,
        post_kind,
    } = &notifications[0].body
    {
        assert_eq!(
            post_kind,
            &PubkyAppPostKind::Collection,
            "Replying to a collection should tag the notification with the parent's post_kind = Collection"
        );
        assert_eq!(
            replied_by, &bob_id,
            "Reply notification should contain the correct replier"
        );
        assert_eq!(
            reply_uri,
            &post_uri_builder(bob_id.clone(), bob_reply_id.clone()),
            "Reply notification should contain the correct reply URI"
        );
        assert_eq!(
            parent_post_uri, &parent_absolute_uri,
            "Reply notification should contain the correct collection URI"
        );
    } else {
        panic!("Expected a Reply notification, found something else");
    }

    // Editing then deleting the reply notifies Alice with the reply's own kind
    // (Short), not the parent collection's.
    let mut edited_reply = reply_post.clone();
    edited_reply.content = "Watcher:ReplyToCollectionNotification:Bob:Reply:Edited".to_string();
    test.put(&bob_kp, &bob_reply_path, &edited_reply).await?;
    let notifications = Notification::get_by_id(&alice_id, Pagination::default())
        .await
        .unwrap();
    assert!(
        matches!(
            &notifications[0].body,
            NotificationBody::PostEdited { edit_source: PostChangedSource::Reply, post_kind, .. }
                if post_kind == &PubkyAppPostKind::Short
        ),
        "Editing a reply to a collection notifies the parent with the reply's post_kind = Short"
    );

    test.cleanup_post(&bob_kp, &bob_reply_path).await?;
    let notifications = Notification::get_by_id(&alice_id, Pagination::default())
        .await
        .unwrap();
    assert!(
        matches!(
            &notifications[0].body,
            NotificationBody::PostDeleted { delete_source: PostChangedSource::Reply, post_kind, .. }
                if post_kind == &PubkyAppPostKind::Short
        ),
        "Deleting a reply to a collection notifies the parent with the reply's post_kind = Short"
    );

    // Cleanup
    test.cleanup_post(&alice_kp, &alice_post_path).await?;
    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;

    Ok(())
}
