use super::utils::find_post_tag;
use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    notification::{Notification, NotificationBody},
    pubky_app::{traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser},
};

#[tokio::test]
async fn test_homeserver_tag_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (post author)
    let author_keypair = Keypair::random();

    let author_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_post_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagPostNotification:Author".to_string(),
        status: None,
    };

    let author_id = test.create_user(&author_keypair, &author_user).await?;

    // Create second user (tagger)
    let tagger_keypair = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_post_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagPostNotification:Tagger".to_string(),
        status: None,
    };
    let tagger_id = test.create_user(&tagger_keypair, &tagger_user).await?;

    // Author creates a post
    let post = PubkyAppPost {
        content: "Watcher:TagPostNotification:Author:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
    };
    let post_id = test.create_post(&author_id, &post).await?;

    // Tagger adds a tag to the post
    let label = "interesting";

    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", author_id, post_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        tagger_id,
        tag.create_id()
    );

    // Put tag
    test.create_tag(tag_url.as_str(), tag_blob).await?;

    // GRAPH_OP
    let post_tag = find_post_tag(&author_id, &post_id, label).await.unwrap();
    assert_eq!(post_tag.label, label);
    assert_eq!(post_tag.taggers_count, 1);
    assert_eq!(post_tag.taggers[0], tagger_id);

    // Check if the author of the post has a new notification
    let notifications = Notification::get_by_id(&author_id, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        1,
        "Post author should have 1 notification for the new tag"
    );

    let notification = &notifications[0];
    if let NotificationBody::TagPost {
        tagged_by,
        tag_label,
        post_uri,
    } = &notification.body
    {
        assert_eq!(
            tagged_by, &tagger_id,
            "Notification should contain the correct tagger"
        );
        assert_eq!(
            tag_label, label,
            "Notification should contain the correct tag label"
        );
        assert_eq!(
            post_uri,
            &format!("pubky://{}/pub/pubky.app/posts/{}", author_id, post_id),
            "Notification should contain the correct post URI"
        );
    } else {
        panic!("Expected a tag post notification, found something else");
    }

    // Cleanup
    test.cleanup_post(&author_id, &post_id).await?;
    test.cleanup_user(&author_id).await?;
    test.cleanup_user(&tagger_id).await?;

    Ok(())
}
