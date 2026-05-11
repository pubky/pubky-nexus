use crate::event_processor::mentions::utils::find_post_mentions;
use crate::event_processor::posts::utils::{
    assert_notification_count, check_member_post_replies, check_member_total_engagement_user_posts,
    find_post_counts, pubky_id, short_post, short_reply, short_repost, test_user,
};
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::RedisOps;
use nexus_common::models::post::{PostCounts, PostDetails, PostRelationships};
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::post_uri_builder;

use super::{
    assert_root_post_fully_indexed, delete_mention_edge, simulate_partial_put_failure_reply,
    simulate_partial_put_failure_root,
};

/// sync_put recovery: simulate a previous attempt that wrote the post to the
/// graph but failed before persisting Redis state. On retry, the handler must
/// re-run idempotent index writes WITHOUT double-incrementing user counts.
#[tokio_shared_rt::test(shared)]
async fn test_post_put_recovers_after_partial_redis_write() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user(
                "Watcher:Post:PutRecovery:User",
                "test_post_put_recovers_after_partial_redis_write",
            ),
        )
        .await?;

    let post = short_post("Watcher:Post:PutRecovery:Post");
    let (post_id, _post_path) = test.create_post(&user_kp, &post).await?;

    // Sanity: post is fully indexed and user count is 1.
    assert_root_post_fully_indexed(&user_id, &post_id).await?;
    assert_eq!(find_user_counts(&user_id).await.posts, 1);

    // Simulate the partial-failure window: graph still has the post but Redis
    // is missing PostDetails / PostRelationships / PostCounts AND all sorted-set
    // memberships. UserCounts has already been incremented by the prior
    // (failed) attempt.
    simulate_partial_put_failure_root(&user_id, &post_id).await?;

    // Retry: invoke sync_put directly. Graph reports `Updated`, the handler
    // takes the recovery path and rebuilds the Redis state from the graph.
    handlers::post::sync_put(post.clone(), pubky_id(&user_id)?, post_id.clone()).await?;

    // PostDetails / PostRelationships / PostCounts and the three root-post
    // sorted-set memberships must all be back.
    assert_root_post_fully_indexed(&user_id, &post_id).await?;

    let recovered_details = PostDetails::get_from_index(&user_id, &post_id)
        .await?
        .expect("PostDetails should be re-indexed during recovery");
    assert_eq!(recovered_details.content, post.content);

    // UserCounts MUST NOT have been double-incremented by the recovery.
    assert_eq!(find_user_counts(&user_id).await.posts, 1);

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Replay sync_put after a full-success first attempt: calling sync_put again
/// with identical content must be a no-op. User counts must not be
/// double-incremented. For replies/reposts, parent sorted-set scores and
/// parent counts must also stay stable.
#[tokio_shared_rt::test(shared)]
async fn test_post_put_replay_after_full_success_is_noop() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user(
                "Watcher:Post:PutReplay:User",
                "test_post_put_replay_after_full_success_is_noop",
            ),
        )
        .await?;

    let post = short_post("Watcher:Post:PutReplay:Post");
    let (post_id, _post_path) = test.create_post(&user_kp, &post).await?;

    // Snapshot the state after the first (full-success) sync_put.
    let details_before = PostDetails::get_from_index(&user_id, &post_id)
        .await?
        .expect("details after first put");
    let counts_before = PostCounts::get_from_index(&user_id, &post_id)
        .await?
        .expect("counts after first put");
    assert_eq!(find_user_counts(&user_id).await.posts, 1);

    // Replay sync_put with identical content. Handler must hit the
    // `existed == Some(matching)` branch and early-return.
    handlers::post::sync_put(post.clone(), pubky_id(&user_id)?, post_id.clone()).await?;

    // User counts must not have been double-incremented.
    assert_eq!(
        find_user_counts(&user_id).await.posts,
        1,
        "UserCounts.posts must not be double-incremented on replay"
    );

    // PostDetails must be unchanged (same indexed_at in particular — no
    // re-stamp that would drift sorted-set scores).
    let details_after = PostDetails::get_from_index(&user_id, &post_id)
        .await?
        .expect("details after replay");
    assert_eq!(details_before.content, details_after.content);
    assert_eq!(
        details_before.indexed_at, details_after.indexed_at,
        "indexed_at must not drift on replay"
    );

    // PostCounts must be unchanged.
    let counts_after = PostCounts::get_from_index(&user_id, &post_id)
        .await?
        .expect("counts after replay");
    assert_eq!(counts_before.replies, counts_after.replies);
    assert_eq!(counts_before.reposts, counts_after.reposts);
    assert_eq!(counts_before.tags, counts_after.tags);

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// sync_put recovery of MENTIONED graph edges: simulate a partial mention
/// loop where the MENTIONED edge was never committed to the graph AND the
/// PostDetails Redis entry is missing. On retry, `recover_post_index_state`
/// must re-MERGE the MENTIONED edge via `merge_mention_edges` WITHOUT
/// re-sending the mention notification (0 > N).
#[tokio_shared_rt::test(shared)]
async fn test_post_put_recovers_mention_edge() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Author (Alice).
    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(
            &alice_kp,
            &test_user(
                "Watcher:Post:PutRecoverMention:Alice",
                "test_post_put_recovers_mention_edge",
            ),
        )
        .await?;

    // Mentioned user (Bob).
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(
            &bob_kp,
            &test_user(
                "Watcher:Post:PutRecoverMention:Bob",
                "test_post_put_recovers_mention_edge",
            ),
        )
        .await?;

    // Alice posts mentioning Bob.
    let post = short_post(format!("Hello pubky{bob_id} — a mention"));
    let (post_id, _post_path) = test.create_post(&alice_kp, &post).await?;

    // Sanity: MENTIONED edge exists in graph, Bob has 1 mention notification.
    let mentioned_before = find_post_mentions(&alice_id, &post_id).await?;
    assert!(
        mentioned_before.contains(&bob_id),
        "Bob MENTIONED by Alice's post after first sync_put"
    );
    assert_notification_count(&bob_id, 1, "1 mention notif after first sync_put").await;

    // Simulate a mid-mention-loop crash state:
    //   - DELETE the MENTIONED edge from graph (as if the MERGE never ran).
    //   - Wipe PostDetails from Redis (forces recovery branch on retry).
    delete_mention_edge(&alice_id, &post_id, &bob_id).await?;
    let post_key: &[&str] = &[alice_id.as_str(), post_id.as_str()];
    PostDetails::remove_from_index_multiple_json(&[post_key]).await?;

    // Retry: graph reports Updated, handler enters recovery path, which
    // calls merge_mention_edges and then reindexes Redis state.
    handlers::post::sync_put(post.clone(), pubky_id(&alice_id)?, post_id.clone()).await?;

    // MENTIONED edge must be back.
    let mentioned_after = find_post_mentions(&alice_id, &post_id).await?;
    assert!(
        mentioned_after.contains(&bob_id),
        "recovery must re-MERGE MENTIONED edge via merge_mention_edges"
    );

    // PostDetails must be re-indexed.
    assert!(PostDetails::get_from_index(&alice_id, &post_id)
        .await?
        .is_some());

    // Bob must NOT have received a duplicate mention notification —
    // merge_mention_edges is notification-free (0 > N on retry).
    assert_notification_count(&bob_id, 1, "no duplicate mention notif on recovery").await;

    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}

/// sync_put recovery for a reply post: a reply takes a different branch in
/// `PostDetails::put_to_index` (the `Some(parent)` branch that writes to the
/// post-reply and replies-per-user sorted sets). This test exercises that
/// branch by wiping the reply's Redis state including its entry in the
/// parent's post-reply sorted set, then verifying recovery rebuilds it.
#[tokio_shared_rt::test(shared)]
async fn test_post_put_recovers_reply_preserves_parent_sorted_sets() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Parent author (Alice).
    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(
            &alice_kp,
            &test_user(
                "Watcher:Post:PutRecoverReply:Alice",
                "test_post_put_recovers_reply_preserves_parent_sorted_sets",
            ),
        )
        .await?;

    // Reply author (Bob).
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(
            &bob_kp,
            &test_user(
                "Watcher:Post:PutRecoverReply:Bob",
                "test_post_put_recovers_reply_preserves_parent_sorted_sets",
            ),
        )
        .await?;

    // Alice's parent post.
    let parent_post = short_post("Watcher:Post:PutRecoverReply:Parent");
    let (parent_id, _parent_path) = test.create_post(&alice_kp, &parent_post).await?;
    let parent_absolute_uri = post_uri_builder(alice_id.clone(), parent_id.clone());

    // Bob's reply.
    let reply_post = short_reply(
        "Watcher:Post:PutRecoverReply:Reply",
        parent_absolute_uri.clone(),
    );
    let (reply_id, _reply_path) = test.create_post(&bob_kp, &reply_post).await?;

    // Sanity: reply fully indexed, parent's post-reply sorted set has the
    // reply member, Alice has 1 notification from Bob's reply.
    let parent_post_key: &[&str; 2] = &[alice_id.as_str(), parent_id.as_str()];
    let reply_member_key: &[&str] = &[bob_id.as_str(), reply_id.as_str()];
    assert!(
        check_member_post_replies(&alice_id, &parent_id, reply_member_key)
            .await?
            .is_some(),
        "reply in parent's post-reply sorted set after creation"
    );
    assert!(PostDetails::get_from_index(&bob_id, &reply_id)
        .await?
        .is_some());
    assert_notification_count(&alice_id, 1, "1 notif after reply creation").await;
    assert_eq!(find_post_counts(&alice_id, &parent_id).await.replies, 1);

    // Simulate a partial-failure window on the reply sync_put: PostDetails/
    // Relationships/Counts wiped from Redis, plus the reply removed from
    // the parent's post-reply sorted set and Bob's replies-per-user sorted
    // set (the writes that PostDetails::put_to_index does in the
    // Some(parent) branch).
    simulate_partial_put_failure_reply(&bob_id, &reply_id, parent_post_key).await?;

    // Retry: graph reports Updated, handler enters recovery path. Recovery
    // calls PostDetails::reindex which re-runs put_to_index — including the
    // Some(parent) branch that re-adds the reply to the parent's post-reply
    // sorted set.
    handlers::post::sync_put(reply_post.clone(), pubky_id(&bob_id)?, reply_id.clone()).await?;

    // Reply's Redis state must be rebuilt.
    assert!(PostDetails::get_from_index(&bob_id, &reply_id)
        .await?
        .is_some());
    assert!(PostRelationships::get_from_index(&bob_id, &reply_id)
        .await?
        .is_some());
    assert!(PostCounts::get_from_index(&bob_id, &reply_id)
        .await?
        .is_some());

    // Parent's post-reply sorted set must contain Bob's reply again.
    assert!(
        check_member_post_replies(&alice_id, &parent_id, reply_member_key)
            .await?
            .is_some(),
        "recovery must re-add reply to parent's post-reply sorted set"
    );

    // Parent's `replies` count must not have been incremented again
    // (reindex from graph reads the live edge count, which is still 1).
    assert_eq!(
        find_post_counts(&alice_id, &parent_id).await.replies,
        1,
        "parent reply count not double-counted on recovery"
    );

    // Bob's user counts must not have been double-incremented.
    assert_eq!(
        find_user_counts(&bob_id).await.posts,
        1,
        "Bob's post count not double-incremented"
    );
    assert_eq!(
        find_user_counts(&bob_id).await.replies,
        1,
        "Bob's reply count not double-incremented"
    );

    // Alice's notification count must be unchanged — recovery does not
    // re-fire `new_post_reply`.
    assert_notification_count(&alice_id, 1, "no duplicate reply notif on recovery").await;

    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}

/// sync_put recovery for a repost: a repost is a root post from the
/// reposter's perspective (is_reply = false) but carries a REPOSTED edge in
/// the graph that PostRelationships::reindex must recover. This test wipes
/// the reposter's Redis state (but leaves the parent's repost count and
/// engagement score untouched) and verifies recovery rebuilds the repost's
/// indexes without double-counting the parent or re-notifying the parent
/// author.
#[tokio_shared_rt::test(shared)]
async fn test_post_put_recovers_repost_preserves_parent_state() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Parent author (Alice).
    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(
            &alice_kp,
            &test_user(
                "Watcher:Post:PutRecoverRepost:Alice",
                "test_post_put_recovers_repost_preserves_parent_state",
            ),
        )
        .await?;

    // Reposter (Bob).
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(
            &bob_kp,
            &test_user(
                "Watcher:Post:PutRecoverRepost:Bob",
                "test_post_put_recovers_repost_preserves_parent_state",
            ),
        )
        .await?;

    // Alice's parent post.
    let parent_post = short_post("Watcher:Post:PutRecoverRepost:Parent");
    let (parent_id, _parent_path) = test.create_post(&alice_kp, &parent_post).await?;
    let parent_absolute_uri = post_uri_builder(alice_id.clone(), parent_id.clone());

    // Bob's repost of Alice's parent.
    let repost = short_repost(
        "Watcher:Post:PutRecoverRepost:Repost",
        parent_absolute_uri.clone(),
    );
    let (repost_id, _repost_path) = test.create_post(&bob_kp, &repost).await?;

    // Sanity: Bob's repost is fully indexed as a root post, parent state and
    // Alice's notification are set.
    let parent_key: &[&str] = &[alice_id.as_str(), parent_id.as_str()];
    assert_root_post_fully_indexed(&bob_id, &repost_id).await?;
    assert_eq!(find_user_counts(&bob_id).await.posts, 1);
    assert_eq!(find_post_counts(&alice_id, &parent_id).await.reposts, 1);
    assert_eq!(
        check_member_total_engagement_user_posts(parent_key).await?,
        Some(1),
        "parent engagement = 1 before simulation"
    );
    assert_notification_count(&alice_id, 1, "1 notif after repost creation").await;

    // Simulate a partial-failure window on the repost's sync_put: wipe
    // everything the reposter's side wrote (PostDetails, PostRelationships,
    // PostCounts, and the three root-post sorted-set memberships). Leave
    // Alice's parent state and Bob's UserCounts alone — the failed attempt
    // is assumed to have already incremented UserCounts.posts and updated
    // Alice's parent.
    simulate_partial_put_failure_root(&bob_id, &repost_id).await?;

    // Retry: graph reports Updated, handler enters recovery path. Recovery
    // rebuilds PostDetails/PostRelationships/PostCounts from graph truth —
    // PostRelationships::reindex must read the REPOSTED edge back into the
    // `reposted` field, and PostCounts::reindex must put the repost back in
    // the engagement sorted set (is_reply = false from graph).
    handlers::post::sync_put(repost.clone(), pubky_id(&bob_id)?, repost_id.clone()).await?;

    // Bob's repost must be fully indexed again as a root post.
    assert_root_post_fully_indexed(&bob_id, &repost_id).await?;

    // PostRelationships.reposted must be recovered from graph.
    let recovered_rel = PostRelationships::get_from_index(&bob_id, &repost_id)
        .await?
        .expect("PostRelationships must be re-indexed during recovery");
    assert!(
        recovered_rel.reposted.is_some(),
        "recovery must re-populate PostRelationships.reposted from graph"
    );

    // Bob's user counts must not have been double-incremented.
    assert_eq!(
        find_user_counts(&bob_id).await.posts,
        1,
        "Bob's post count not double-incremented"
    );

    // Alice's parent state must be unchanged — recovery only touches the
    // repost's own indexes, not the parent's counters or engagement score.
    assert_eq!(
        find_post_counts(&alice_id, &parent_id).await.reposts,
        1,
        "parent repost count not double-counted on recovery"
    );
    assert_eq!(
        check_member_total_engagement_user_posts(parent_key).await?,
        Some(1),
        "parent engagement unchanged after recovery"
    );

    // Alice's notification count must be unchanged — recovery does not
    // re-fire `new_repost`.
    assert_notification_count(&alice_id, 1, "no duplicate repost notif on recovery").await;

    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}
