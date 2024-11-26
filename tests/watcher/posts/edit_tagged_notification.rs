use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky_app_specs::{traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser};
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};

#[tokio::test]
async fn test_edit_tagged_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create User A who makes the original post
    let keypair_a = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:TaggedPostEditNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&keypair_a, &user_a).await?;

    // Create User B who tags User A's post
    let keypair_b = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:TaggedPostEditNotification:UserB".to_string(),
        status: None,
    };
    let user_b_id = test.create_user(&keypair_b, &user_b).await?;

    // User A creates a post
    let mut post = PubkyAppPost {
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
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_a_id, post_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_id = tag.create_id();
    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", user_b_id, tag_id);

    // Put tag
    test.create_tag(&tag_url, tag_blob).await?;

    // User A edits their post
    post.content = "Edited post by User A".to_string();
    let edited_post_blob = serde_json::to_vec(&post)?;
    let edited_url = format!("pubky://{}/pub/pubky.app/posts/{}", user_a_id, post_id);

    // Overwrite existing post in the homeserver with the edited one
    test.client
        .put(edited_url.as_str(), &edited_post_blob)
        .await?;
    test.ensure_event_processing_complete().await?;

    // Verify that User B receives a notification about the edit
    let notifications = Notification::get_by_id(&user_b_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        1,
        "User B should have exactly one notification"
    );

    if let NotificationBody::PostEdited {
        edit_source,
        edited_by,
        edited_uri,
        linked_uri,
    } = &notifications[0].body
    {
        assert_eq!(
            edited_by, &user_a_id,
            "Notification should specify the correct user who edited the post"
        );
        assert_eq!(
            edited_uri,
            &format!("pubky://{}/pub/pubky.app/posts/{}", user_a_id, post_id),
            "Notification should contain the correct edited post URI"
        );
        assert_eq!(
            linked_uri, &tag_url,
            "Notification should contain the correct tag URI"
        );
        assert_eq!(
            edit_source,
            &PostChangedSource::TaggedPost,
            "Edit notification should have the correct source"
        );
    } else {
        panic!("Expected a PostEdited notification, found something else");
    }

    Ok(())
}
