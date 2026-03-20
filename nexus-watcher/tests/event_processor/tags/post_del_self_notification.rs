use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::{models::notification::Notification, types::Pagination};
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_self_untag_post_no_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a single user who will be both author and tagger
    let user_kp = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_self_untag_post_no_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:SelfUntagPostNotification:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // User creates a post
    let post = PubkyAppPost {
        content: "Watcher:SelfUntagPostNotification:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // User tags their own post
    let label = "self_tagged";

    let tag = PubkyAppTag {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();

    // Put tag (self-tag should not produce a TagPost notification)
    test.put(&user_kp, &tag_path, tag).await?;

    let notifications = Notification::get_by_id(&user_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        0,
        "Self-tagging own post should not produce a notification"
    );

    // Delete the tag (self-untag should not produce an UntagPost notification)
    test.del(&user_kp, &tag_path).await?;

    let notifications = Notification::get_by_id(&user_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        0,
        "Self-untagging own post should not produce a notification"
    );

    // Cleanup
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
