use anyhow::Result;
use pubky::Keypair;

use super::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
    check_member_user_post_timeline, collection_post, find_post_counts, find_post_details,
    test_user,
};
use crate::event_processor::utils::watcher::WatcherTest;

/// PUT a `kind=collection` post and confirm:
/// - the graph node exists with `kind="collection"`,
/// - PostCounts is initialized,
/// - the post is NOT in the global timeline sorted set,
/// - the post is NOT in the per-user timeline sorted set,
/// - the post is NOT in the global engagement sorted set.
///
/// This is the load-bearing assertion for the Phase 3 stream-suppression
/// strategy: collections appear in the graph (so they can be queried via the
/// Cypher fallback with `?kind=collection`) but never enter the Redis index
/// sorted sets that power the default Home / Hot / By-Tag streams.
#[tokio_shared_rt::test(shared)]
async fn test_create_collection_post_indexes_graph_only() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = test_user("Watcher:Collection:User", "test_collection_post");
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = collection_post(
        "AI papers",
        Some("Best stuff"),
        vec![
            format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52A"),
            format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52B"),
            format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52C"),
        ],
    );

    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Graph: the collection node exists with kind="collection".
    let post_details = find_post_details(&user_id, &post_id).await?;
    assert_eq!(post_details.id, post_id);
    assert_eq!(post_details.kind, pubky_app_specs::PubkyAppPostKind::Collection);
    assert_eq!(post_details.attachments.as_ref().unwrap().len(), 3);

    // PostCounts initialized at zero.
    let counts = find_post_counts(&user_id, &post_id).await;
    assert_eq!(counts.replies, 0);
    assert_eq!(counts.reposts, 0);
    assert_eq!(counts.tags, 0);

    // Sorted sets must NOT include the collection.
    let global_timeline = check_member_global_timeline_user_post(&user_id, &post_id).await?;
    assert!(
        global_timeline.is_none(),
        "collection must not be in global timeline sorted set"
    );
    let per_user_timeline = check_member_user_post_timeline(&user_id, &post_id).await?;
    assert!(
        per_user_timeline.is_none(),
        "collection must not be in per-user timeline sorted set"
    );
    let engagement = check_member_total_engagement_user_posts(&[&user_id, &post_id]).await?;
    assert!(
        engagement.is_none(),
        "collection must not be in global engagement sorted set"
    );

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Edit a `kind=collection` post (rename, add an item) and confirm:
/// - the graph node reflects the edit,
/// - the post still does NOT appear in any of the suppressed sorted sets.
#[tokio_shared_rt::test(shared)]
async fn test_edit_collection_post_keeps_streams_suppressed() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = test_user("Watcher:Collection:Edit:User", "edit_collection_post");
    let user_id = test.create_user(&user_kp, &user).await?;

    // Create with 3 items.
    let original_attachments = vec![
        format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52A"),
        format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52B"),
        format!("pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52C"),
    ];
    let post = collection_post(
        "Original name",
        Some("Original description"),
        original_attachments.clone(),
    );
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Edit: rename + add a 4th item. Re-PUT to the same path.
    let mut edited_attachments = original_attachments.clone();
    edited_attachments.push(format!(
        "pubky://{user_id}/pub/pubky.app/posts/0034A0X7NJ52D"
    ));
    let edited = collection_post(
        "Renamed",
        Some("Updated description"),
        edited_attachments.clone(),
    );
    test.put(&user_kp, &post_path, &edited).await?;

    // Graph reflects the edit.
    let post_details = find_post_details(&user_id, &post_id).await?;
    assert_eq!(post_details.attachments.as_ref().unwrap().len(), 4);

    // Sorted sets still do not contain the collection.
    assert!(
        check_member_global_timeline_user_post(&user_id, &post_id)
            .await?
            .is_none(),
        "edited collection must stay out of global timeline"
    );
    assert!(
        check_member_user_post_timeline(&user_id, &post_id)
            .await?
            .is_none(),
        "edited collection must stay out of per-user timeline"
    );
    assert!(
        check_member_total_engagement_user_posts(&[&user_id, &post_id])
            .await?
            .is_none(),
        "edited collection must stay out of engagement set"
    );

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// DEL a `kind=collection` post that has no surviving relationships and
/// confirm the graph node is removed (no orphan sorted-set entries).
#[tokio_shared_rt::test(shared)]
async fn test_delete_collection_post_removes_graph_no_orphans() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = test_user("Watcher:Collection:Del:User", "del_collection_post");
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = collection_post("Drafts", None, vec![]);
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Confirm the collection was created.
    let _ = find_post_details(&user_id, &post_id).await?;

    // Delete and confirm the graph node is gone.
    test.cleanup_post(&user_kp, &post_path).await?;

    let result = find_post_details(&user_id, &post_id).await;
    assert!(
        result.is_err(),
        "deleted collection must not be in the graph"
    );

    // Sorted sets had no entries to begin with — confirm still empty.
    assert!(
        check_member_global_timeline_user_post(&user_id, &post_id)
            .await?
            .is_none()
    );
    assert!(
        check_member_total_engagement_user_posts(&[&user_id, &post_id])
            .await?
            .is_none()
    );

    test.cleanup_user(&user_kp).await?;
    Ok(())
}
