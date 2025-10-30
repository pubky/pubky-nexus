use super::utils::find_post_tag;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::notification::{Notification, NotificationBody},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder,
    traits::{HasIdPath, HashId},
    PubkyAppPost, PubkyAppTag, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_tag_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (post author)
    let author_kp = Keypair::random();

    let author_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_post_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagPostNotification:Author".to_string(),
        status: None,
    };

    let author_id = test.create_user(&author_kp, &author_user).await?;

    // Create second user (tagger)
    let tagger_kp = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_post_notification".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagPostNotification:Tagger".to_string(),
        status: None,
    };
    let tagger_id = test.create_user(&tagger_kp, &tagger_user).await?;

    // Author creates a post
    let post = PubkyAppPost {
        content: "Watcher:TagPostNotification:Author:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&author_kp, &post).await?;

    // Tagger adds a tag to the post
    let label = "interesting";

    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_relative_url = PubkyAppTag::create_path(&tag.create_id());

    // Put tag
    test.put(&tagger_kp, &tag_relative_url, tag).await?;

    // GRAPH_OP
    let post_tag = find_post_tag(&author_id, &post_id, label)
        .await
        .unwrap()
        .expect("Failed to find post tag in graph database");

    assert_eq!(post_tag.label, label);
    assert_eq!(post_tag.taggers_count, 1);
    assert_eq!(post_tag.taggers[0], tagger_id);

    // Check if the author of the post has a new notification
    let notifications = Notification::get_by_id(&author_id, Pagination::default())
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
            &format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"),
            "Notification should contain the correct post URI"
        );
    } else {
        panic!("Expected a tag post notification, found something else");
    }

    // Cleanup
    test.cleanup_post(&author_kp, &post_id).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}
