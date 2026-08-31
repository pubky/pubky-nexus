use super::utils::collection_post;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::{
    models::notification::{Notification, NotificationBody, PostChangedSource},
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{
    bookmark_uri_builder, post_uri_builder, traits::HashId, PubkyAppBookmark, PubkyAppPostKind,
    PubkyAppUser,
};

/// Deleting a bookmarked collection (soft-delete tombstone) must still report
/// the prior kind.
#[tokio_shared_rt::test(shared)]
async fn test_delete_bookmarked_collection_notification() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create User A who makes the original collection
    let user_a_kp = Keypair::random();
    let user_a = PubkyAppUser {
        bio: Some("User A bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:CollectionDeleteNotification:UserA".to_string(),
        status: None,
    };
    let user_a_id = test.create_user(&user_a_kp, &user_a).await?;

    // Create User B who bookmarks User A's collection
    let user_b_kp = Keypair::random();
    let user_b = PubkyAppUser {
        bio: Some("User B bio".to_string()),
        image: None,
        links: None,
        name: "Watcher:CollectionDeleteNotification:UserB".to_string(),
        status: None,
    };
    let user_b_id = test.create_user(&user_b_kp, &user_b).await?;

    // User A creates a collection
    let post = collection_post("doomed");
    let (post_id, post_path) = test.create_post(&user_a_kp, &post).await?;

    // User B bookmarks User A's collection
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(user_a_id.clone(), post_id.clone()),
        created_at: 0,
    };
    let bookmark_absolute_url = bookmark_uri_builder(user_b_id.clone(), bookmark.create_id());
    let bookmark_path = bookmark.hs_path();
    test.put(&user_b_kp, &bookmark_path, bookmark).await?;

    // User A deletes their collection
    test.cleanup_post(&user_a_kp, &post_path).await?;

    // Verify that User B receives a collection-specific delete notification
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
        post_kind,
    } = &notifications[0].body
    {
        assert_eq!(
            post_kind,
            &PubkyAppPostKind::Collection,
            "Deleting a collection should tag the notification with post_kind = Collection (prior kind, via the tombstone)"
        );
        assert_eq!(
            deleted_by, &user_a_id,
            "Notification should specify the correct user who deleted the collection"
        );
        assert_eq!(
            deleted_uri,
            &format!("pubky://{user_a_id}/pub/pubky.app/posts/{post_id}"),
            "Notification should contain the correct deleted collection URI"
        );
        assert_eq!(
            linked_uri, &bookmark_absolute_url,
            "Notification should contain the correct bookmark URI"
        );
        assert_eq!(
            delete_source,
            &PostChangedSource::Bookmark,
            "Delete notification should have the correct source"
        );
    } else {
        panic!("Expected a PostDeleted notification, found something else");
    }

    Ok(())
}
