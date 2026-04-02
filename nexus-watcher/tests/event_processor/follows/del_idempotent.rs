use super::utils::find_follow_relationship;
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    db::kv::JsonAction,
    db::RedisOps,
    models::{
        follow::{Followers, Following, UserFollows},
        user::UserCounts,
    },
};
use nexus_watcher::events::handlers::follow;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyId};

/// Test that calling sync_del twice (simulating a retry) does not produce
/// negative counts or corrupt indexes.
#[tokio_shared_rt::test(shared)]
async fn test_follow_del_idempotent() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

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
    let follower_pubky = PubkyId::try_from(follower_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
    let followee_pubky = PubkyId::try_from(followee_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
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
    let mut test = WatcherTest::setup().await?;

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
    let follower_pubky = PubkyId::try_from(follower_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
    let followee_pubky = PubkyId::try_from(followee_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
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

/// Test that retrying an unfollow between friends does not double-decrement
/// the friends counter for either user.
#[tokio_shared_rt::test(shared)]
async fn test_follow_del_friends_idempotent() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

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
    let a_pubky = PubkyId::try_from(a_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
    let b_pubky = PubkyId::try_from(b_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
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
