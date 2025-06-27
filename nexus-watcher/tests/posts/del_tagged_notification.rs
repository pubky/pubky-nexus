use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_delete_tagged_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create User A who makes the original post
    let keypair_a = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:TaggedPostDeleteNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&keypair_a, &user_a).await?;

    // Create User B who tags User A's post
    let keypair_b = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:TaggedPostDeleteNotification:UserB".to_string(),
        status: None,
    };
    let user_b_id = test.create_user(&keypair_b, &user_b).await?;

    // User A creates a post
    let post = PubkyAppPost {
        content: "Original post by User A".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_a_id, &post).await?;

    // User B tags User A's post
    let label = "merkle_tree";
    let tag = PubkyAppTag {
        uri: format!("pubky://{user_a_id}/pub/pubky.app/posts/{post_id}"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    let tag_url = format!("pubky://{user_b_id}/pub/pubky.app/tags/{tag_id}");

    // Put tag
    test.put(&tag_url, tag).await?;

    // User A deletes their post
    test.cleanup_post(&user_a_id, &post_id).await?;

    // Verify that User B receives a notification about the deletion
    let notifications = Notification::get_by_id(&user_b_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        1,
        "User B should have exactly one notification"
    );

    if let NotificationBody::PostDeleted {
        delete_source,
        deleted_by,
        deleted_uri,
        linked_uri,
    } = &notifications[0].body
    {
        assert_eq!(
            deleted_by, &user_a_id,
            "Notification should specify the correct user who deleted the post"
        );
        assert_eq!(
            deleted_uri,
            &format!("pubky://{user_a_id}/pub/pubky.app/posts/{post_id}"),
            "Notification should contain the correct deleted post URI"
        );
        assert_eq!(
            linked_uri, &tag_url,
            "Notification should contain the correct tag URI"
        );
        assert_eq!(
            delete_source,
            &PostChangedSource::TaggedPost,
            "Delete notification should have the correct source"
        );
    } else {
        panic!("Expected a PostDeleted notification, found something else");
    }

    Ok(())
}
