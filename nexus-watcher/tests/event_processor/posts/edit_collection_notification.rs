use super::utils::collection_post;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, tag_uri_builder, traits::HashId, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_edit_tagged_collection_notification() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create User A who makes the original collection
    let user_a_kp = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:CollectionEditNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&user_a_kp, &user_a).await?;

    // Create User B who tags User A's collection
    let user_b_kp = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:CollectionEditNotification:UserB".to_string(),
        status: None,
    };
    let user_b_id = test.create_user(&user_b_kp, &user_b).await?;

    // User A creates a collection
    let mut post = collection_post("original");
    let (post_id, post_path) = test.create_post(&user_a_kp, &post).await?;

    // User B tags User A's collection
    let label = "merkle_tree";
    let tag = PubkyAppTag {
        uri: post_uri_builder(user_a_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    let tag_id = tag.create_id();
    let tag_absolute_url = tag_uri_builder(user_b_id.clone(), tag_id);
    test.put(&user_b_kp, &tag_path, tag).await?;

    // User A edits their collection
    post.content = serde_json::json!({ "name": "renamed", "items": [] }).to_string();
    test.put(&user_a_kp, &post_path, &post).await?;

    // Verify that User B receives a collection-specific edit notification
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
        post_kind,
    } = &notifications[0].body
    {
        assert_eq!(
            post_kind,
            &PubkyAppPostKind::Collection,
            "Editing a collection should tag the notification with post_kind = Collection"
        );
        assert_eq!(
            edited_by, &user_a_id,
            "Notification should specify the correct user who edited the collection"
        );
        assert_eq!(
            edited_uri,
            &format!("pubky://{user_a_id}/pub/pubky.app/posts/{post_id}"),
            "Notification should contain the correct edited collection URI"
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

    // Kind-toggle rule: an edit branches on the NEW kind. Editing the
    // collection into a Short must report post_kind = Short.
    post.kind = PubkyAppPostKind::Short;
    post.content = "No longer a collection".to_string();
    test.put(&user_a_kp, &post_path, &post).await?;

    let notifications = Notification::get_by_id(&user_b_id, Pagination::default())
        .await
        .unwrap();

    assert_eq!(
        notifications.len(),
        2,
        "User B should have two notifications after the second edit"
    );

    // Notifications are returned in descending order, the toggle edit is first
    assert!(
        matches!(
            &notifications[0].body,
            NotificationBody::PostEdited { post_kind, .. } if post_kind == &PubkyAppPostKind::Short
        ),
        "Editing a collection into a Short should report post_kind = Short (new-kind rule)"
    );

    Ok(())
}
