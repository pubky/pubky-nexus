use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, tag_uri_builder, traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_edit_tagged_post_notification() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create User A who makes the original post
    let user_a_kp = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:TaggedPostEditNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&user_a_kp, &user_a).await?;

    // Create User B who tags User A's post
    let user_b_kp = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:TaggedPostEditNotification:UserB".to_string(),
        status: None,
    };
    let user_b_id = test.create_user(&user_b_kp, &user_b).await?;

    // User A creates a post
    let mut post = PubkyAppPost {
        content: "Original post by User A".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&user_a_kp, &post).await?;

    // User B tags User A's post
    let label = "merkle_tree";
    let tag = PubkyAppTag {
        uri: post_uri_builder(user_a_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    let tag_id = tag.create_id();
    let tag_absolute_url = tag_uri_builder(user_b_id.clone(), tag_id);

    // Put tag
    test.put(&user_b_kp, &tag_path, tag).await?;

    // User A edits their post
    post.content = "Edited post by User A".to_string();

    // Overwrite existing post in the homeserver with the edited one
    test.put(&user_a_kp, &post_path, &post).await?;

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
            &format!("pubky://{user_a_id}/pub/pubky.app/posts/{post_id}"),
            "Notification should contain the correct edited post URI"
        );
        assert_eq!(
            linked_uri, &tag_absolute_url,
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
