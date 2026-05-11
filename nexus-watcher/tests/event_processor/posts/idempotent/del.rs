use crate::event_processor::posts::utils::{
    assert_notification_count, check_member_total_engagement_user_posts, find_post_counts,
    find_post_details, pubky_id, short_post, short_reply, short_repost, test_user,
};
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::event::EventProcessorError;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::post_uri_builder;

use super::{simulate_partial_del_cleanup_child, simulate_partial_del_cleanup_root, ChildKind};

/// sync_del graph-last recovery: simulate a previous attempt that completed
/// every Redis cleanup step but failed before deleting the graph node. On
/// retry, the handler must idempotently re-run cleanup, leave UserCounts at 0
/// (no double-decrement), and finally delete the graph node.
#[tokio_shared_rt::test(shared)]
async fn test_post_del_recovers_after_partial_redis_cleanup() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user(
                "Watcher:Post:DelRecovery:User",
                "test_post_del_recovers_after_partial_redis_cleanup",
            ),
        )
        .await?;

    let post = short_post("Watcher:Post:DelRecovery:Post");
    let (post_id, _post_path) = test.create_post(&user_kp, &post).await?;

    // Sanity: fully indexed.
    assert!(find_post_details(&user_id, &post_id).await.is_ok());
    assert_eq!(find_user_counts(&user_id).await.posts, 1);

    // Simulate the partial-failure state of the new graph-last sync_del:
    //   - Gate (PostRelationships) atomically removed
    //   - UserCounts decremented (gate was true on the failed attempt)
    //   - PostCounts + PostDetails Redis entries removed
    //   - Graph node still present (graph delete is the LAST step)
    simulate_partial_del_cleanup_root(&user_id, &post_id).await?;

    // Sanity: graph still has the post (graph delete is the final step).
    assert!(find_post_details(&user_id, &post_id).await.is_ok());

    // Retry through the public entry point: post::del re-checks
    // post_is_safe_to_delete and dispatches into sync_del again.
    handlers::post::del(pubky_id(&user_id)?, post_id.clone()).await?;

    // Final state: graph node gone (retry ran its sole remaining step — the
    // graph delete), count NOT double-decremented (still 0, not -1 — retry
    // saw `post_in_index = false` and skipped `UserCounts::decrement`).
    assert!(find_post_details(&user_id, &post_id).await.is_err());
    assert_eq!(find_user_counts(&user_id).await.posts, 0);

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Replay sync_del on a fully deleted post: post::del should report
/// MissingDependency (mapped to SkipIndexing) without corrupting state.
#[tokio_shared_rt::test(shared)]
async fn test_post_del_replay_after_full_success_skips() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user(
                "Watcher:Post:DelReplay:User",
                "test_post_del_replay_after_full_success_skips",
            ),
        )
        .await?;

    let post = short_post("Watcher:Post:DelReplay:Post");
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Delete through the normal event flow.
    test.cleanup_post(&user_kp, &post_path).await?;

    assert!(find_post_details(&user_id, &post_id).await.is_err());
    assert_eq!(find_user_counts(&user_id).await.posts, 0);

    // Replay: graph is gone, post_is_safe_to_delete returns no rows ->
    // MissingDependency -> SkipIndexing.
    let result = handlers::post::del(pubky_id(&user_id)?, post_id.clone()).await;
    assert!(
        matches!(result, Err(EventProcessorError::SkipIndexing)),
        "Replay after full delete should return SkipIndexing, got: {result:?}"
    );

    // State must remain clean.
    assert_eq!(find_user_counts(&user_id).await.posts, 0);

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// sync_del recovery for a reply post: simulate the same partial-cleanup
/// scenario for a post that has a parent (reply). The parent's reply count,
/// parent's engagement sorted-set score, and parent author's notification
/// count MUST NOT be double-mutated on retry.
#[tokio_shared_rt::test(shared)]
async fn test_post_del_reply_recovers_without_double_decrement() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Parent author (Alice).
    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(
            &alice_kp,
            &test_user(
                "Watcher:Post:DelReplyRecovery:Alice",
                "test_post_del_reply_recovers_without_double_decrement",
            ),
        )
        .await?;

    // Reply author (Bob).
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(
            &bob_kp,
            &test_user(
                "Watcher:Post:DelReplyRecovery:Bob",
                "test_post_del_reply_recovers_without_double_decrement",
            ),
        )
        .await?;

    // Alice's parent post.
    let parent_post = short_post("Watcher:Post:DelReplyRecovery:Parent");
    let (parent_id, _parent_path) = test.create_post(&alice_kp, &parent_post).await?;
    let parent_absolute_uri = post_uri_builder(alice_id.clone(), parent_id.clone());

    // Bob's reply to Alice's parent.
    let reply_post = short_reply(
        "Watcher:Post:DelReplyRecovery:Reply",
        parent_absolute_uri.clone(),
    );
    let (reply_id, _reply_path) = test.create_post(&bob_kp, &reply_post).await?;

    // Sanity: parent has 1 reply, parent's engagement score is 1, Alice has
    // 1 notification (from Bob's reply creation).
    let parent_key: &[&str] = &[alice_id.as_str(), parent_id.as_str()];
    assert_eq!(find_post_counts(&alice_id, &parent_id).await.replies, 1);
    assert_eq!(find_user_counts(&bob_id).await.replies, 1);
    assert_eq!(
        check_member_total_engagement_user_posts(parent_key).await?,
        Some(1),
        "parent engagement = 1 after reply creation"
    );
    assert_notification_count(&alice_id, 1, "1 notif after reply creation").await;

    // Simulate a fully-completed Redis cleanup of sync_del where only the
    // graph delete failed at the very end.
    simulate_partial_del_cleanup_child(&bob_id, &reply_id, &alice_id, &parent_id, ChildKind::Reply)
        .await?;

    // Retry through the public entry point.
    handlers::post::del(pubky_id(&bob_id)?, reply_id.clone()).await?;

    // Reply graph node gone; parent reply count NOT decremented again.
    assert!(find_post_details(&bob_id, &reply_id).await.is_err());
    assert_eq!(
        find_post_counts(&alice_id, &parent_id).await.replies,
        0,
        "parent reply count not double-decremented on retry"
    );
    assert_eq!(
        find_user_counts(&bob_id).await.replies,
        0,
        "Bob's reply count not double-decremented on retry"
    );
    // Parent engagement score must not go negative — retry saw
    // `post_in_index = false` and skipped `decrement_score_index_sorted_set`.
    assert_eq!(
        check_member_total_engagement_user_posts(parent_key).await?,
        Some(0),
        "parent engagement not double-decremented on retry"
    );
    // Alice's notification count must not have grown — retry saw
    // `post_in_index = false` and skipped the `post_children_changed` fire.
    assert_notification_count(&alice_id, 1, "no duplicate reply-del notif on retry").await;

    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}

/// sync_del recovery for a repost post: simulate a partial cleanup of a
/// repost where every Redis mutation completed but the graph delete failed.
/// The parent post's `reposts` count MUST NOT be double-decremented on retry,
/// and the parent author's notification set MUST NOT receive a duplicate
/// repost-deletion notification.
#[tokio_shared_rt::test(shared)]
async fn test_post_del_repost_recovers_without_double_decrement() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Parent author (Alice).
    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(
            &alice_kp,
            &test_user(
                "Watcher:Post:DelRepostRecovery:Alice",
                "test_post_del_repost_recovers_without_double_decrement",
            ),
        )
        .await?;

    // Reposter (Bob).
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(
            &bob_kp,
            &test_user(
                "Watcher:Post:DelRepostRecovery:Bob",
                "test_post_del_repost_recovers_without_double_decrement",
            ),
        )
        .await?;

    // Alice's parent post.
    let parent_post = short_post("Watcher:Post:DelRepostRecovery:Parent");
    let (parent_id, _parent_path) = test.create_post(&alice_kp, &parent_post).await?;
    let parent_absolute_uri = post_uri_builder(alice_id.clone(), parent_id.clone());

    // Bob's repost of Alice's parent.
    let repost = short_repost(
        "Watcher:Post:DelRepostRecovery:Repost",
        parent_absolute_uri.clone(),
    );
    let (repost_id, _repost_path) = test.create_post(&bob_kp, &repost).await?;

    // Sanity: parent has 1 repost, engagement score 1, Alice has 1 notification.
    let parent_key: &[&str] = &[alice_id.as_str(), parent_id.as_str()];
    assert_eq!(find_post_counts(&alice_id, &parent_id).await.reposts, 1);
    assert_eq!(
        check_member_total_engagement_user_posts(parent_key).await?,
        Some(1),
        "parent engagement = 1 after repost creation"
    );
    assert_notification_count(&alice_id, 1, "1 notif after repost creation").await;

    // Simulate a fully-completed Redis cleanup of sync_del where only the
    // graph delete failed at the very end.
    simulate_partial_del_cleanup_child(
        &bob_id,
        &repost_id,
        &alice_id,
        &parent_id,
        ChildKind::Repost,
    )
    .await?;

    // Retry through the public entry point.
    handlers::post::del(pubky_id(&bob_id)?, repost_id.clone()).await?;

    // Repost graph node gone; parent repost count NOT decremented again.
    assert!(find_post_details(&bob_id, &repost_id).await.is_err());
    assert_eq!(
        find_post_counts(&alice_id, &parent_id).await.reposts,
        0,
        "parent repost count not double-decremented on retry"
    );
    assert_eq!(
        find_user_counts(&bob_id).await.posts,
        0,
        "Bob's post count not double-decremented on retry"
    );
    // Parent engagement score must not go negative — retry saw
    // `post_in_index = false` and skipped `decrement_score_index_sorted_set`.
    assert_eq!(
        check_member_total_engagement_user_posts(parent_key).await?,
        Some(0),
        "parent engagement not double-decremented on retry"
    );
    // Alice's notification count must not have grown — retry saw
    // `post_in_index = false` and skipped the `post_children_changed` fire.
    assert_notification_count(&alice_id, 1, "no duplicate repost-del notif on retry").await;

    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}
