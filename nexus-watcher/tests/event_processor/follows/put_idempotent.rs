use super::utils::find_follow_relationship;
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    db::RedisOps,
    models::{
        follow::{Followers, Following, UserFollows},
        notification::Notification,
    },
    types::Pagination,
};
use nexus_watcher::events::handlers::follow;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyId};

/// Test that calling sync_put twice (simulating a retry) does not double
/// counters, duplicate index entries, or create extra notifications.
#[tokio_shared_rt::test(shared)]
async fn test_follow_put_idempotent() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create follower
    let follower_kp = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("test_follow_put_idempotent".to_string()),
        image: None,
        links: None,
        name: "Watcher:IdempotentPut:Follower".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_kp, &follower_user).await?;

    // Create followee
    let followee_kp = Keypair::random();
    let followee_user = PubkyAppUser {
        bio: Some("test_follow_put_idempotent".to_string()),
        image: None,
        links: None,
        name: "Watcher:IdempotentPut:Followee".to_string(),
        status: None,
    };
    let followee_id = test.create_user(&followee_kp, &followee_user).await?;

    // First follow (normal flow through event processing)
    test.create_follow(&follower_kp, &followee_id).await?;

    // Verify initial state: counts = 1
    let followee_counts = find_user_counts(&followee_id).await;
    assert_eq!(
        followee_counts.followers, 1,
        "Followee should have 1 follower"
    );
    let follower_counts = find_user_counts(&follower_id).await;
    assert_eq!(
        follower_counts.following, 1,
        "Follower should be following 1"
    );

    // Verify index membership
    let (_, is_follower) = Followers::check_set_member(&[&followee_id], &follower_id).await?;
    assert!(is_follower, "Follower should be in followee's follower set");
    let (_, is_following) = Following::check_set_member(&[&follower_id], &followee_id).await?;
    assert!(
        is_following,
        "Followee should be in follower's following set"
    );

    // Count notifications before retry
    let notifications_before = Notification::get_by_id(&followee_id, Pagination::default()).await?;
    let notification_count_before = notifications_before.len();

    // Simulate retry: call sync_put directly with the same follower/followee
    let follower_pubky = PubkyId::try_from(follower_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
    let followee_pubky = PubkyId::try_from(followee_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
    follow::sync_put(follower_pubky, followee_pubky).await?;

    // Verify counts are unchanged (not doubled)
    let followee_counts = find_user_counts(&followee_id).await;
    assert_eq!(
        followee_counts.followers, 1,
        "Followee should still have 1 follower after retry"
    );
    let follower_counts = find_user_counts(&follower_id).await;
    assert_eq!(
        follower_counts.following, 1,
        "Follower should still be following 1 after retry"
    );

    // Verify index membership is unchanged
    let (_, is_follower) = Followers::check_set_member(&[&followee_id], &follower_id).await?;
    assert!(
        is_follower,
        "Follower should still be in followee's follower set after retry"
    );
    let (_, is_following) = Following::check_set_member(&[&follower_id], &followee_id).await?;
    assert!(
        is_following,
        "Followee should still be in follower's following set after retry"
    );

    // Verify no duplicate notifications
    let notifications_after = Notification::get_by_id(&followee_id, Pagination::default()).await?;
    assert_eq!(
        notifications_after.len(),
        notification_count_before,
        "No new notifications should be created on retry"
    );

    // Verify graph relationship still exists
    let exists = find_follow_relationship(&follower_id, &followee_id).await?;
    assert!(exists, "Follow relationship should still exist in graph");

    // Cleanup
    test.cleanup_user(&follower_kp).await?;
    test.cleanup_user(&followee_kp).await?;

    Ok(())
}

/// Test partial failure recovery: graph edge was created on a previous attempt
/// but index writes failed. On retry, sync_put should recover both indexes
/// without duplicating counters or notifications.
#[tokio_shared_rt::test(shared)]
async fn test_follow_put_recovers_missing_indexes() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create follower
    let follower_kp = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("test_follow_put_recovers_missing_indexes".to_string()),
        image: None,
        links: None,
        name: "Watcher:RecoverPut:Follower".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_kp, &follower_user).await?;

    // Create followee
    let followee_kp = Keypair::random();
    let followee_user = PubkyAppUser {
        bio: Some("test_follow_put_recovers_missing_indexes".to_string()),
        image: None,
        links: None,
        name: "Watcher:RecoverPut:Followee".to_string(),
        status: None,
    };
    let followee_id = test.create_user(&followee_kp, &followee_user).await?;

    // Normal follow (graph + indexes + counters all complete)
    test.create_follow(&follower_kp, &followee_id).await?;

    // Simulate partial failure: remove both index entries as if they were never written
    let followers = Followers(vec![follower_id.to_string()]);
    let following = Following(vec![followee_id.to_string()]);
    followers.del_from_index(&followee_id).await?;
    following.del_from_index(&follower_id).await?;

    // Verify indexes are gone
    let (_, is_follower) = Followers::check_set_member(&[&followee_id], &follower_id).await?;
    assert!(!is_follower, "Follower index should be missing");
    let (_, is_following) = Following::check_set_member(&[&follower_id], &followee_id).await?;
    assert!(!is_following, "Following index should be missing");

    // Record counters and notifications before recovery
    let counts_before = find_user_counts(&followee_id).await;
    let notifications_before = Notification::get_by_id(&followee_id, Pagination::default()).await?;

    // Simulate retry: sync_put hits Updated (graph edge exists) and runs recovery
    let follower_pubky = PubkyId::try_from(follower_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
    let followee_pubky = PubkyId::try_from(followee_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
    follow::sync_put(follower_pubky, followee_pubky).await?;

    // Verify both indexes are recovered
    let (_, is_follower) = Followers::check_set_member(&[&followee_id], &follower_id).await?;
    assert!(
        is_follower,
        "Follower index should be recovered after retry"
    );
    let (_, is_following) = Following::check_set_member(&[&follower_id], &followee_id).await?;
    assert!(
        is_following,
        "Following index should be recovered after retry"
    );

    // Verify counters were NOT incremented again
    let counts_after = find_user_counts(&followee_id).await;
    assert_eq!(
        counts_before.followers, counts_after.followers,
        "Follower count should not change on recovery"
    );

    // Verify no duplicate notifications
    let notifications_after = Notification::get_by_id(&followee_id, Pagination::default()).await?;
    assert_eq!(
        notifications_before.len(),
        notifications_after.len(),
        "No new notifications should be created on recovery"
    );

    // Cleanup
    test.cleanup_user(&follower_kp).await?;
    test.cleanup_user(&followee_kp).await?;

    Ok(())
}
