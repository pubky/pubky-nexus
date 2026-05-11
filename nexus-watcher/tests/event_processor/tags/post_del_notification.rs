use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::notification::{Notification, NotificationBody},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_untag_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (post author)
    let author_kp = Keypair::random();

    let author_user = PubkyAppUser {
        bio: Some("test_homeserver_untag_post_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:UntagPostNotification:Author".to_string(),
        status: None,
    };

    let author_id = test.create_user(&author_kp, &author_user).await?;

    // Create second user (tagger)
    let tagger_kp = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_untag_post_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:UntagPostNotification:Tagger".to_string(),
        status: None,
    };
    let tagger_id = test.create_user(&tagger_kp, &tagger_user).await?;

    // Author creates a post
    let post = PubkyAppPost {
        content: "Watcher:UntagPostNotification:Author:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    // Tagger adds a tag to the post
    let label = "remarkable";

    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_path = tag.hs_path();

    // Put tag
    test.put(&tagger_kp, &tag_path, tag).await?;

    // Verify the TagPost notification was created
    let notifications = Notification::get_by_id(&author_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        1,
        "Post author should have 1 notification for the new tag"
    );

    // Delete the tag
    test.del(&tagger_kp, &tag_path).await?;

    // Check if the author of the post now has a new UntagPost notification
    let notifications = Notification::get_by_id(&author_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        2,
        "Post author should have 2 notifications: tag + untag"
    );

    // The most recent notification (first in descending order) should be UntagPost
    let notification = &notifications[0];
    if let NotificationBody::UntagPost {
        untagged_by,
        tag_label,
        post_uri,
    } = &notification.body
    {
        assert_eq!(
            untagged_by, &tagger_id,
            "Notification should contain the correct untagger"
        );
        assert_eq!(
            tag_label, label,
            "Notification should contain the correct tag label"
        );
        assert_eq!(
            post_uri,
            &format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"),
            "Notification should contain the correct post URI"
        );
    } else {
        panic!("Expected an untag post notification, found something else");
    }

    // Cleanup
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}
