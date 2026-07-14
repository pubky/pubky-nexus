use super::utils::collection_post;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_repost_collection_notification() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let alice_kp = Keypair::random();
    let alice = PubkyAppUser {
        bio: Some("test_repost_collection_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:RepostCollectionNotification:Alice".to_string(),
        status: None,
    };
    let alice_id = test.create_user(&alice_kp, &alice).await?;

    // Alice creates a collection
    let collection = collection_post("reposted");
    let (alice_post_id, alice_post_path) = test.create_post(&alice_kp, &collection).await?;
    let embed_absolute_uri = post_uri_builder(alice_id.clone(), alice_post_id.clone());

    let bob_kp = Keypair::random();
    let bob = PubkyAppUser {
        bio: Some("test_repost_collection_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:RepostCollectionNotification:Bob".to_string(),
        status: None,
    };
    let bob_id = test.create_user(&bob_kp, &bob).await?;

    // embed.kind lies (Short) though the target is a Collection: the indexed kind wins.
    let bob_repost = PubkyAppPost {
        content: "Watcher:RepostCollectionNotification:Bob:Repost".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: embed_absolute_uri.clone(),
        }),
        attachments: None,
        lock: None,
    };
    let (bob_repost_id, bob_repost_path) = test.create_post(&bob_kp, &bob_repost).await?;

    // Verify that Alice gets a collection-specific repost notification
    let notifications = Notification::get_by_id(&alice_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        1,
        "Alice should have exactly one notification from Bob's repost"
    );

    if let NotificationBody::Repost {
        reposted_by,
        embed_uri,
        repost_uri,
        post_kind,
    } = &notifications[0].body
    {
        assert_eq!(
            post_kind,
            &PubkyAppPostKind::Collection,
            "Reposting a collection should tag the notification with the embed's post_kind = Collection"
        );
        assert_eq!(
            reposted_by, &bob_id,
            "Repost notification should contain the correct reposter"
        );
        assert_eq!(
            embed_uri, &embed_absolute_uri,
            "Repost notification should contain the correct collection URI"
        );
        assert_eq!(
            repost_uri,
            &post_uri_builder(bob_id.clone(), bob_repost_id.clone()),
            "Repost notification should contain the correct repost URI"
        );
    } else {
        panic!("Expected a Repost notification, found something else");
    }

    // Cleanup
    test.cleanup_post(&bob_kp, &bob_repost_path).await?;
    test.cleanup_post(&alice_kp, &alice_post_path).await?;
    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;

    Ok(())
}
