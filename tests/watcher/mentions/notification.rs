use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    models::{
        notification::{Notification, NotificationBody},
        pubky_app::{PostKind, PubkyAppPost, PubkyAppUser},
    },
    types::Pagination,
};

#[tokio::test]
async fn test_homeserver_mentions_notifications() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (author)
    let author_keypair = Keypair::random();
    let author_user = PubkyAppUser {
        bio: Some("test_homeserver_mentions_notifications".to_string()),
        image: None,
        links: None,
        name: "Watcher:MentionsNotification:Author".to_string(),
        status: None,
    };
    let author_user_id = test.create_user(&author_keypair, &author_user).await?;

    // Create second user (mention 1)
    let mentioned_user_1_keypair = Keypair::random();
    let mentioned_user_1 = PubkyAppUser {
        bio: Some("test_homeserver_mentions".to_string()),
        image: None,
        links: None,
        name: "Watcher:MentionsNotification:MentionedUser1".to_string(),
        status: None,
    };
    let mentioned_user_1_id = test
        .create_user(&mentioned_user_1_keypair, &mentioned_user_1)
        .await?;

    // Create third user (mention 2)
    let mentioned_user_2_keypair = Keypair::random();
    let mentioned_user_2 = PubkyAppUser {
        bio: Some("test_homeserver_mentions".to_string()),
        image: None,
        links: None,
        name: "Watcher:MentionsNotification:MentionedUser2".to_string(),
        status: None,
    };
    let mentioned_user_2_id = test
        .create_user(&mentioned_user_2_keypair, &mentioned_user_2)
        .await?;

    // User 1 writes a post mentioning User 2 and User 3
    let post_content = format!(
        "This is a post mentioning pk:{}, and pk:{}",
        mentioned_user_1_id, mentioned_user_2_id
    );
    let post = PubkyAppPost {
        content: post_content.clone(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&author_user_id, &post).await?;

    // Check if mentioned User 1 received a Mention notification
    let notifications_1 = Notification::get_by_id(&mentioned_user_1_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications_1.len(),
        1,
        "Mentioned User 1 should have 1 notification"
    );

    let notification_1 = &notifications_1[0];
    if let NotificationBody::Mention {
        mentioned_by,
        post_uri,
    } = &notification_1.body
    {
        assert_eq!(
            mentioned_by, &author_user_id,
            "Notification should contain the correct mentioner"
        );
        assert_eq!(
            post_uri,
            &format!("pubky://{}/pub/pubky.app/posts/{}", author_user_id, post_id),
            "Notification should contain the correct post URI"
        );
    } else {
        panic!("Expected a Mention notification, found something else");
    }

    // Check if mentioned User 2 received a Mention notification
    let notifications_2 = Notification::get_by_id(&mentioned_user_2_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications_2.len(),
        1,
        "Mentioned User 2 should have 1 notification"
    );

    let notification_2 = &notifications_2[0];
    if let NotificationBody::Mention {
        mentioned_by,
        post_uri,
    } = &notification_2.body
    {
        assert_eq!(
            mentioned_by, &author_user_id,
            "Notification should contain the correct mentioner"
        );
        assert_eq!(
            post_uri,
            &format!("pubky://{}/pub/pubky.app/posts/{}", author_user_id, post_id),
            "Notification should contain the correct post URI"
        );
    } else {
        panic!("Expected a Mention notification, found something else");
    }

    // Cleanup
    test.cleanup_post(&author_user_id, &post_id).await?;
    test.cleanup_user(&author_user_id).await?;
    test.cleanup_user(&mentioned_user_1_id).await?;
    test.cleanup_user(&mentioned_user_2_id).await?;

    Ok(())
}
