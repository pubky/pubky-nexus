use super::utils::find_follow_relationship;
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    db::kv::{guards, JsonAction},
    db::RedisOps,
    models::{
        follow::{Followers, Following, UserFollows},
        user::UserCounts,
    },
};
use nexus_watcher::events::handlers::follow;
use nexus_watcher::events::handlers::utils::{follow_deletion_guard_key, DELETION_GUARD_TTL_SECS};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyId};

/// Test that calling sync_del twice (simulating a retry) does not produce
/// negative counts or corrupt indexes.
#[tokio_shared_rt::test(shared)]
async fn test_follow_del_idempotent() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create follower
    let follower_kp = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("test_follow_del_idempotent".to_string()),
        image: None,
        links: None,
        name: "Watcher:IdempotentDel:Follower".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_kp, &follower_user).await?;

    // Create followee
    let followee_kp = Keypair::random();
    let followee_user = PubkyAppUser {
        bio: Some("test_follow_del_idempotent".to_string()),
        image: None,
        links: None,
        name: "Watcher:IdempotentDel:Followee".to_string(),
        status: None,
    };
    let followee_id = test.create_user(&followee_kp, &followee_user).await?;

    // Create follow and then unfollow (normal flow)
    let follow_url = test.create_follow(&follower_kp, &followee_id).await?;
    test.del(&follower_kp, &follow_url).await?;

    // Verify initial state after unfollow: counts = 0
    let followee_counts = find_user_counts(&followee_id).await;
    assert_eq!(
        followee_counts.followers, 0,
        "Followee should have 0 followers"
    );
    let follower_counts = find_user_counts(&follower_id).await;
    assert_eq!(
        follower_counts.following, 0,
        "Follower should be following 0"
    );

    // Verify index membership cleared
    let (_, is_follower) = Followers::check_set_member(&[&followee_id], &follower_id).await?;
    assert!(
        !is_follower,
        "Follower should not be in followee's follower set"
    );
    let (_, is_following) = Following::check_set_member(&[&follower_id], &followee_id).await?;
    assert!(
        !is_following,
        "Followee should not be in follower's following set"
    );

    // Verify graph relationship removed
    let exists = find_follow_relationship(&follower_id, &followee_id).await?;
    assert!(!exists, "Follow relationship should not exist in graph");

    // Simulate retry: call sync_del directly with the same follower/followee
    let follower_pubky = PubkyId::from(follower_kp.clone());
    let followee_pubky = PubkyId::from(followee_kp.clone());
    follow::sync_del(follower_pubky, followee_pubky).await?;

    // Verify counts are still 0 (not negative)
    let followee_counts = find_user_counts(&followee_id).await;
    assert_eq!(
        followee_counts.followers, 0,
        "Followee should still have 0 followers after retry"
    );
    let follower_counts = find_user_counts(&follower_id).await;
    assert_eq!(
        follower_counts.following, 0,
        "Follower should still be following 0 after retry"
    );

    // Verify index membership still cleared
    let (_, is_follower) = Followers::check_set_member(&[&followee_id], &follower_id).await?;
    assert!(
        !is_follower,
        "Follower should still not be in followee's follower set after retry"
    );
    let (_, is_following) = Following::check_set_member(&[&follower_id], &followee_id).await?;
    assert!(
        !is_following,
        "Followee should still not be in follower's following set after retry"
    );

    // Cleanup
    test.cleanup_user(&follower_kp).await?;
    test.cleanup_user(&followee_kp).await?;

    Ok(())
}

/// Test partial failure recovery: the original DEL attempt failed before SREM
/// and counter decrement ran, so indexes and counters still reflect the PUT
/// state (stale). On retry, sync_del should clean up indexes and decrement
/// counters exactly once (from 1 to 0), not double-decrement.
#[tokio_shared_rt::test(shared)]
async fn test_follow_del_recovers_stale_indexes() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create follower
    let follower_kp = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("test_follow_del_recovers_stale_indexes".to_string()),
        image: None,
        links: None,
        name: "Watcher:RecoverDel:Follower".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_kp, &follower_user).await?;

    // Create followee
    let followee_kp = Keypair::random();
    let followee_user = PubkyAppUser {
        bio: Some("test_follow_del_recovers_stale_indexes".to_string()),
        image: None,
        links: None,
        name: "Watcher:RecoverDel:Followee".to_string(),
        status: None,
    };
    let followee_id = test.create_user(&followee_kp, &followee_user).await?;

    // Create follow and then unfollow (normal flow — everything completes)
    let follow_url = test.create_follow(&follower_kp, &followee_id).await?;
    test.del(&follower_kp, &follow_url).await?;

    // Verify clean state: counts = 0, indexes cleared
    let followee_counts = find_user_counts(&followee_id).await;
    assert_eq!(followee_counts.followers, 0);
    let follower_counts = find_user_counts(&follower_id).await;
    assert_eq!(follower_counts.following, 0);

    // Simulate partial failure: restore indexes and counters to their PUT state
    // as if the original DEL never ran SREM or counter decrement.
    let followers = Followers(vec![follower_id.to_string()]);
    let following = Following(vec![followee_id.to_string()]);
    followers.put_to_index(&followee_id).await?;
    following.put_to_index(&follower_id).await?;
    UserCounts::update_index_field(&followee_id, "followers", JsonAction::Increment(1)).await?;
    UserCounts::update_index_field(&follower_id, "following", JsonAction::Increment(1)).await?;

    // Verify stale state: indexes present, counters = 1
    let (_, is_follower) = Followers::check_set_member(&[&followee_id], &follower_id).await?;
    assert!(is_follower, "Stale follower index should be present");
    let (_, is_following) = Following::check_set_member(&[&follower_id], &followee_id).await?;
    assert!(is_following, "Stale following index should be present");
    let followee_counts = find_user_counts(&followee_id).await;
    assert_eq!(
        followee_counts.followers, 1,
        "Followee should have 1 follower (stale)"
    );
    let follower_counts = find_user_counts(&follower_id).await;
    assert_eq!(
        follower_counts.following, 1,
        "Follower should be following 1 (stale)"
    );

    // Simulate retry: sync_del sees stale indexes, decrements counters once, cleans up
    let follower_pubky = PubkyId::from(follower_kp.clone());
    let followee_pubky = PubkyId::from(followee_kp.clone());
    follow::sync_del(follower_pubky, followee_pubky).await?;

    // Verify both stale indexes are cleaned up
    let (_, is_follower) = Followers::check_set_member(&[&followee_id], &follower_id).await?;
    assert!(
        !is_follower,
        "Stale follower index should be cleaned up after retry"
    );
    let (_, is_following) = Following::check_set_member(&[&follower_id], &followee_id).await?;
    assert!(
        !is_following,
        "Stale following index should be cleaned up after retry"
    );

    // Verify counts decremented from 1 to 0 (exactly once, not double-decremented)
    let followee_counts = find_user_counts(&followee_id).await;
    assert_eq!(
        followee_counts.followers, 0,
        "Follower count should be 0 after recovery (was 1)"
    );
    let follower_counts = find_user_counts(&follower_id).await;
    assert_eq!(
        follower_counts.following, 0,
        "Following count should be 0 after recovery (was 1)"
    );

    // Cleanup
    test.cleanup_user(&follower_kp).await?;
    test.cleanup_user(&followee_kp).await?;

    Ok(())
}

/// Tombstone gate vs read-through resurrection, follow flavor: attempt 1
/// of deleting the F->X follow completed every Redis step (both follow sets
/// SREMed, both counters decremented once) but failed at the final graph
/// delete. Between attempts, `Followers::get_by_id(X)` read-through
/// re-populates `Followers:{X}` from the still-present graph edge, which is
/// the exact set the retry uses as its `still_indexed` gate. With the
/// tombstone held by attempt 1, the retry must skip the non-idempotent
/// decrements: F's `following` count is decremented exactly once across both
/// attempts.
///
/// F follows a SECOND user (Y) so `following` starts at 2: the counter
/// decrement is floored at 0, so starting from 1 a double decrement would be
/// invisible (0 stays 0) and the headline assertion could never fail on the
/// unguarded code path.
#[tokio_shared_rt::test(shared)]
async fn test_follow_del_retry_skips_side_effects_after_readthrough_resurrection() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Follower F
    let f_kp = Keypair::random();
    let f_user = PubkyAppUser {
        bio: Some(
            "test_follow_del_retry_skips_side_effects_after_readthrough_resurrection".to_string(),
        ),
        image: None,
        links: None,
        name: "Watcher:DelResurrection:Follower".to_string(),
        status: None,
    };
    let f_id = test.create_user(&f_kp, &f_user).await?;

    // Followee X (the follow under deletion; F is X's only follower)
    let x_kp = Keypair::random();
    let x_user = PubkyAppUser {
        bio: Some(
            "test_follow_del_retry_skips_side_effects_after_readthrough_resurrection".to_string(),
        ),
        image: None,
        links: None,
        name: "Watcher:DelResurrection:FolloweeX".to_string(),
        status: None,
    };
    let x_id = test.create_user(&x_kp, &x_user).await?;

    // Followee Y (keeps F's `following` count above the floor)
    let y_kp = Keypair::random();
    let y_user = PubkyAppUser {
        bio: Some(
            "test_follow_del_retry_skips_side_effects_after_readthrough_resurrection".to_string(),
        ),
        image: None,
        links: None,
        name: "Watcher:DelResurrection:FolloweeY".to_string(),
        status: None,
    };
    let y_id = test.create_user(&y_kp, &y_user).await?;

    // F follows both X and Y.
    test.create_follow(&f_kp, &x_id).await?;
    test.create_follow(&f_kp, &y_id).await?;

    // Sanity: F is following 2 users, X has exactly 1 follower (F).
    assert_eq!(find_user_counts(&f_id).await.following, 2);
    assert_eq!(find_user_counts(&x_id).await.followers, 1);

    // Simulate attempt 1 of the tombstone-gated sync_del for F->X: the guard
    // was acquired, both follow sets were SREMed, both counters were
    // decremented once (F and X are not friends, so no friends update), and
    // only the final graph delete failed.
    let guard_key = follow_deletion_guard_key(&f_id, &x_id);
    assert!(
        guards::try_acquire(&guard_key, DELETION_GUARD_TTL_SECS).await?,
        "fresh deletion guard should be acquirable"
    );
    Followers(vec![f_id.to_string()])
        .del_from_index(&x_id)
        .await?;
    Following(vec![x_id.to_string()])
        .del_from_index(&f_id)
        .await?;
    UserCounts::update_index_field(&f_id, "following", JsonAction::Decrement(1)).await?;
    UserCounts::update(&x_id, "followers", JsonAction::Decrement(1), None).await?;

    // Graph edge still present: the graph delete runs LAST and it failed.
    assert!(
        find_follow_relationship(&f_id, &x_id).await?,
        "graph edge should survive the failed first attempt"
    );

    // Between attempts, a read-through resurrects the gate: `Followers:{X}` is
    // re-populated from the still-present graph edge.
    assert!(
        Followers::get_by_id(&x_id, None, None).await?.is_some(),
        "read-through should find X's followers in the graph"
    );
    assert!(
        Followers::check_in_index(&x_id, &f_id).await?,
        "read-through must have resurrected the still_indexed gate"
    );

    // Retry. Without the tombstone this would observe the resurrected gate and
    // decrement `following`/`followers` a second time.
    follow::sync_del(
        PubkyId::from(f_kp.public_key()),
        PubkyId::from(x_kp.public_key()),
    )
    .await?;

    // Graph edge gone; F's `following` decremented exactly once across both
    // attempts (2 -> 1). An unguarded retry would have decremented again (1 -> 0).
    assert!(
        !find_follow_relationship(&f_id, &x_id).await?,
        "graph edge should be gone after the retry"
    );
    assert_eq!(
        find_user_counts(&f_id).await.following,
        1,
        "following count must not be double-decremented after read-through resurrection"
    );
    assert_eq!(
        find_user_counts(&x_id).await.followers,
        0,
        "X should have 0 followers after the delete"
    );

    // The F->Y follow is untouched.
    assert!(
        Following::check_in_index(&f_id, &y_id).await?,
        "F->Y follow should be unaffected"
    );

    // Successful completion released the tombstone: it must be acquirable again.
    assert!(
        guards::try_acquire(&guard_key, DELETION_GUARD_TTL_SECS).await?,
        "deletion guard should have been released after the successful delete"
    );
    guards::release(&guard_key).await?;

    // Cleanup
    test.cleanup_user(&f_kp).await?;
    test.cleanup_user(&x_kp).await?;
    test.cleanup_user(&y_kp).await?;

    Ok(())
}

/// Test that retrying an unfollow between friends does not double-decrement
/// the friends counter for either user.
#[tokio_shared_rt::test(shared)]
async fn test_follow_del_friends_idempotent() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create user A
    let a_kp = Keypair::random();
    let a_user = PubkyAppUser {
        bio: Some("test_follow_del_friends_idempotent".to_string()),
        image: None,
        links: None,
        name: "Watcher:FriendsDel:A".to_string(),
        status: None,
    };
    let a_id = test.create_user(&a_kp, &a_user).await?;

    // Create user B
    let b_kp = Keypair::random();
    let b_user = PubkyAppUser {
        bio: Some("test_follow_del_friends_idempotent".to_string()),
        image: None,
        links: None,
        name: "Watcher:FriendsDel:B".to_string(),
        status: None,
    };
    let b_id = test.create_user(&b_kp, &b_user).await?;

    // Mutual follow: A→B and B→A (makes them friends)
    let a_follows_b = test.create_follow(&a_kp, &b_id).await?;
    test.create_follow(&b_kp, &a_id).await?;

    // Verify friendship: both have friends = 1
    let a_counts = find_user_counts(&a_id).await;
    assert_eq!(a_counts.friends, 1, "A should have 1 friend");
    let b_counts = find_user_counts(&b_id).await;
    assert_eq!(b_counts.friends, 1, "B should have 1 friend");

    // A unfollows B (breaks friendship)
    test.del(&a_kp, &a_follows_b).await?;

    // Verify friendship broken: both have friends = 0
    let a_counts = find_user_counts(&a_id).await;
    assert_eq!(
        a_counts.friends, 0,
        "A should have 0 friends after unfollow"
    );
    assert_eq!(a_counts.following, 0, "A should be following 0");
    let b_counts = find_user_counts(&b_id).await;
    assert_eq!(
        b_counts.friends, 0,
        "B should have 0 friends after unfollow"
    );
    assert_eq!(b_counts.followers, 0, "B should have 0 followers");

    // Simulate retry: call sync_del again for A→B
    let a_pubky = PubkyId::from(a_kp.public_key());
    let b_pubky = PubkyId::from(b_kp.public_key());
    follow::sync_del(a_pubky, b_pubky).await?;

    // Verify counts unchanged (friends not double-decremented)
    let a_counts = find_user_counts(&a_id).await;
    assert_eq!(
        a_counts.friends, 0,
        "A should still have 0 friends after retry"
    );
    assert_eq!(
        a_counts.following, 0,
        "A following should still be 0 after retry"
    );
    let b_counts = find_user_counts(&b_id).await;
    assert_eq!(
        b_counts.friends, 0,
        "B should still have 0 friends after retry"
    );
    assert_eq!(
        b_counts.followers, 0,
        "B followers should still be 0 after retry"
    );

    // B's follow toward A should be unaffected
    let b_counts = find_user_counts(&b_id).await;
    assert_eq!(b_counts.following, 1, "B should still be following 1 (A)");
    let a_counts = find_user_counts(&a_id).await;
    assert_eq!(a_counts.followers, 1, "A should still have 1 follower (B)");

    // Cleanup
    test.cleanup_user(&a_kp).await?;
    test.cleanup_user(&b_kp).await?;

    Ok(())
}
