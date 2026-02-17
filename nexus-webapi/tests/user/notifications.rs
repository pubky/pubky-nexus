use crate::utils::get_request;
use anyhow::Result;
use nexus_common::{
    db::RedisOps,
    models::notification::{Notification, NotificationBody},
};

async fn env_init() {
    crate::utils::server::TestServiceServer::get_test_server().await;
}

/// Inserts a single follow notification into the Redis sorted set with an explicit
/// timestamp, bypassing `new_follow` which would use `Utc::now()` internally.
async fn seed_follow(recipient_id: &str, follower_id: &str, timestamp: i64) -> Result<()> {
    let body = NotificationBody::Follow {
        followed_by: follower_id.to_string(),
    };
    let json = serde_json::to_string(&body).unwrap();
    Notification::put_index_sorted_set(
        &["Notification", recipient_id],
        &[(timestamp as f64, json.as_str())],
        None,
        None,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Seeds 3 follow notifications (A oldest → C newest) for a dedicated test user, then
/// verifies that limit=2 returns exactly the 2 newest items in the correct descending
/// order with correct body content.
///
/// Idempotent: Redis sorted sets deduplicate by member value, so reinserting the same
/// NotificationBody JSON merely refreshes the score (timestamp). Each run produces the
/// same 3-member set — no cleanup needed.
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_with_limit() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "test_notif_limit_recipient_00000000001";
    const FOLLOWER_A: &str = "test_notif_limit_follower_a_00000000001";
    const FOLLOWER_B: &str = "test_notif_limit_follower_b_00000000001";
    const FOLLOWER_C: &str = "test_notif_limit_follower_c_00000000001";

    seed_follow(TEST_USER, FOLLOWER_A, 1000).await?;
    seed_follow(TEST_USER, FOLLOWER_B, 2000).await?;
    seed_follow(TEST_USER, FOLLOWER_C, 3000).await?;

    // Fetch all notifications (default limit=20) — expect exactly 3.
    let all = get_request(&format!("/v0/user/{TEST_USER}/notifications")).await?;
    assert_eq!(
        all.as_array().unwrap().len(),
        3,
        "Expected exactly 3 seeded notifications"
    );

    // Fetch with limit=2 — must return the 2 newest (C, B) newest-first.
    let limited = get_request(&format!("/v0/user/{TEST_USER}/notifications?limit=2")).await?;
    let limited_items = limited.as_array().unwrap();

    assert_eq!(limited_items.len(), 2);

    assert_eq!(limited_items[0]["timestamp"], 3000_i64);
    assert_eq!(limited_items[0]["body"]["type"], "follow");
    assert_eq!(limited_items[0]["body"]["followed_by"], FOLLOWER_C);

    assert_eq!(limited_items[1]["timestamp"], 2000_i64);
    assert_eq!(limited_items[1]["body"]["type"], "follow");
    assert_eq!(limited_items[1]["body"]["followed_by"], FOLLOWER_B);

    Ok(())
}

/// Seeds 3 notifications and verifies that limit=0 returns an empty array.
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_with_limit_zero() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "test_notif_limit_zero_recipient_00000000001";
    const FOLLOWER_A: &str = "test_notif_limit_zero_follower_a_00000000001";
    const FOLLOWER_B: &str = "test_notif_limit_zero_follower_b_00000000001";
    const FOLLOWER_C: &str = "test_notif_limit_zero_follower_c_00000000001";

    seed_follow(TEST_USER, FOLLOWER_A, 1000).await?;
    seed_follow(TEST_USER, FOLLOWER_B, 2000).await?;
    seed_follow(TEST_USER, FOLLOWER_C, 3000).await?;

    let res = get_request(&format!("/v0/user/{TEST_USER}/notifications?limit=0")).await?;
    assert_eq!(
        res.as_array().unwrap().len(),
        0,
        "limit=0 should return empty array"
    );

    Ok(())
}

/// Seeds 5 follow notifications (A oldest → E newest) for a dedicated test user, then
/// verifies that skip=3 skips the 3 newest (E, D, C) and returns the remaining 2 (B, A)
/// in descending order with correct body content.
///
/// Idempotent for the same reason as the limit test.
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_with_skip() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "test_notif_skip_recipient_00000000001";
    const FOLLOWER_A: &str = "test_notif_skip_follower_a_00000000001";
    const FOLLOWER_B: &str = "test_notif_skip_follower_b_00000000001";
    const FOLLOWER_C: &str = "test_notif_skip_follower_c_00000000001";
    const FOLLOWER_D: &str = "test_notif_skip_follower_d_00000000001";
    const FOLLOWER_E: &str = "test_notif_skip_follower_e_00000000001";

    seed_follow(TEST_USER, FOLLOWER_A, 1000).await?;
    seed_follow(TEST_USER, FOLLOWER_B, 2000).await?;
    seed_follow(TEST_USER, FOLLOWER_C, 3000).await?;
    seed_follow(TEST_USER, FOLLOWER_D, 4000).await?;
    seed_follow(TEST_USER, FOLLOWER_E, 5000).await?;

    // Fetch all — expect exactly 5.
    let all = get_request(&format!("/v0/user/{TEST_USER}/notifications")).await?;
    assert_eq!(
        all.as_array().unwrap().len(),
        5,
        "Expected exactly 5 seeded notifications"
    );

    // Fetch with skip=3 — must skip the 3 newest (E, D, C) and return B then A.
    let skipped_res = get_request(&format!("/v0/user/{TEST_USER}/notifications?skip=3")).await?;
    let skipped_items = skipped_res.as_array().unwrap();

    assert_eq!(skipped_items.len(), 2);

    assert_eq!(skipped_items[0]["timestamp"], 2000_i64);
    assert_eq!(skipped_items[0]["body"]["type"], "follow");
    assert_eq!(skipped_items[0]["body"]["followed_by"], FOLLOWER_B);

    assert_eq!(skipped_items[1]["timestamp"], 1000_i64);
    assert_eq!(skipped_items[1]["body"]["type"], "follow");
    assert_eq!(skipped_items[1]["body"]["followed_by"], FOLLOWER_A);

    Ok(())
}

/// Seeds 3 notifications (A=1000, B=2000, C=3000), then verifies that start=2000
/// returns exactly B and A (all notifications with timestamp <= 2000).
///
/// Note: `start` is the upper-bound score (max), `end` is the lower-bound score (min),
/// mirroring Redis ZREVRANGEBYSCORE which reads newest-first from `max` down to `min`.
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_with_start() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "test_notif_start_recipient_00000000001";
    const FOLLOWER_A: &str = "test_notif_start_follower_a_00000000001";
    const FOLLOWER_B: &str = "test_notif_start_follower_b_00000000001";
    const FOLLOWER_C: &str = "test_notif_start_follower_c_00000000001";

    seed_follow(TEST_USER, FOLLOWER_A, 1000).await?;
    seed_follow(TEST_USER, FOLLOWER_B, 2000).await?;
    seed_follow(TEST_USER, FOLLOWER_C, 3000).await?;

    // start=2000 sets max_score=2000 — must return B(2000) and A(1000), newest-first.
    let res = get_request(&format!("/v0/user/{TEST_USER}/notifications?start=2000")).await?;
    let items = res.as_array().unwrap();

    assert_eq!(items.len(), 2);

    assert_eq!(items[0]["timestamp"], 2000_i64);
    assert_eq!(items[0]["body"]["type"], "follow");
    assert_eq!(items[0]["body"]["followed_by"], FOLLOWER_B);

    assert_eq!(items[1]["timestamp"], 1000_i64);
    assert_eq!(items[1]["body"]["type"], "follow");
    assert_eq!(items[1]["body"]["followed_by"], FOLLOWER_A);

    Ok(())
}

/// Seeds 3 notifications (A=1000, B=2000, C=3000), then verifies that end=2000
/// returns exactly C and B (all notifications with timestamp >= 2000).
///
/// Note: `end` is the lower-bound score (min), so `end=2000` means min_score=2000,
/// returning all items at or above that score.
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_with_end() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "test_notif_end_recipient_00000000001";
    const FOLLOWER_A: &str = "test_notif_end_follower_a_00000000001";
    const FOLLOWER_B: &str = "test_notif_end_follower_b_00000000001";
    const FOLLOWER_C: &str = "test_notif_end_follower_c_00000000001";

    seed_follow(TEST_USER, FOLLOWER_A, 1000).await?;
    seed_follow(TEST_USER, FOLLOWER_B, 2000).await?;
    seed_follow(TEST_USER, FOLLOWER_C, 3000).await?;

    // end=2000 sets min_score=2000 — must return C(3000) and B(2000), newest-first.
    let res = get_request(&format!("/v0/user/{TEST_USER}/notifications?end=2000")).await?;
    let items = res.as_array().unwrap();

    assert_eq!(items.len(), 2);

    assert_eq!(items[0]["timestamp"], 3000_i64);
    assert_eq!(items[0]["body"]["type"], "follow");
    assert_eq!(items[0]["body"]["followed_by"], FOLLOWER_C);

    assert_eq!(items[1]["timestamp"], 2000_i64);
    assert_eq!(items[1]["body"]["type"], "follow");
    assert_eq!(items[1]["body"]["followed_by"], FOLLOWER_B);

    Ok(())
}

/// Seeds 5 notifications (A=1000 … E=5000), then verifies that start=4000&end=2000
/// returns exactly D, C, B — the 3 notifications within that inclusive timestamp range.
///
/// Note: `start` is max_score (upper bound) and `end` is min_score (lower bound), so
/// start=4000&end=2000 means the window [2000, 4000].
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_with_start_and_end() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "test_notif_start_end_recipient_00000000001";
    const FOLLOWER_A: &str = "test_notif_start_end_follower_a_00000000001";
    const FOLLOWER_B: &str = "test_notif_start_end_follower_b_00000000001";
    const FOLLOWER_C: &str = "test_notif_start_end_follower_c_00000000001";
    const FOLLOWER_D: &str = "test_notif_start_end_follower_d_00000000001";
    const FOLLOWER_E: &str = "test_notif_start_end_follower_e_00000000001";

    seed_follow(TEST_USER, FOLLOWER_A, 1000).await?;
    seed_follow(TEST_USER, FOLLOWER_B, 2000).await?;
    seed_follow(TEST_USER, FOLLOWER_C, 3000).await?;
    seed_follow(TEST_USER, FOLLOWER_D, 4000).await?;
    seed_follow(TEST_USER, FOLLOWER_E, 5000).await?;

    // start=4000&end=2000 — max_score=4000, min_score=2000 — must return D(4000), C(3000), B(2000), newest-first.
    let res = get_request(&format!(
        "/v0/user/{TEST_USER}/notifications?start=4000&end=2000"
    ))
    .await?;
    let items = res.as_array().unwrap();

    assert_eq!(items.len(), 3);

    assert_eq!(items[0]["timestamp"], 4000_i64);
    assert_eq!(items[0]["body"]["type"], "follow");
    assert_eq!(items[0]["body"]["followed_by"], FOLLOWER_D);

    assert_eq!(items[1]["timestamp"], 3000_i64);
    assert_eq!(items[1]["body"]["type"], "follow");
    assert_eq!(items[1]["body"]["followed_by"], FOLLOWER_C);

    assert_eq!(items[2]["timestamp"], 2000_i64);
    assert_eq!(items[2]["body"]["type"], "follow");
    assert_eq!(items[2]["body"]["followed_by"], FOLLOWER_B);

    Ok(())
}

/// Seeds 5 notifications (A=1000 … E=5000) and verifies that limit=2&skip=1 skips
/// the newest (E) and returns D and C.
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_with_limit_and_skip() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "test_notif_limit_skip_recipient_00000000001";
    const FOLLOWER_A: &str = "test_notif_limit_skip_follower_a_00000000001";
    const FOLLOWER_B: &str = "test_notif_limit_skip_follower_b_00000000001";
    const FOLLOWER_C: &str = "test_notif_limit_skip_follower_c_00000000001";
    const FOLLOWER_D: &str = "test_notif_limit_skip_follower_d_00000000001";
    const FOLLOWER_E: &str = "test_notif_limit_skip_follower_e_00000000001";

    seed_follow(TEST_USER, FOLLOWER_A, 1000).await?;
    seed_follow(TEST_USER, FOLLOWER_B, 2000).await?;
    seed_follow(TEST_USER, FOLLOWER_C, 3000).await?;
    seed_follow(TEST_USER, FOLLOWER_D, 4000).await?;
    seed_follow(TEST_USER, FOLLOWER_E, 5000).await?;

    // Fetch all — expect exactly 5.
    let all = get_request(&format!("/v0/user/{TEST_USER}/notifications")).await?;
    assert_eq!(
        all.as_array().unwrap().len(),
        5,
        "Expected exactly 5 seeded notifications"
    );

    // limit=2&skip=1 — skips E(5000), returns D(4000) and C(3000).
    let res = get_request(&format!(
        "/v0/user/{TEST_USER}/notifications?limit=2&skip=1"
    ))
    .await?;
    let items = res.as_array().unwrap();

    assert_eq!(items.len(), 2);

    assert_eq!(items[0]["timestamp"], 4000_i64);
    assert_eq!(items[0]["body"]["type"], "follow");
    assert_eq!(items[0]["body"]["followed_by"], FOLLOWER_D);

    assert_eq!(items[1]["timestamp"], 3000_i64);
    assert_eq!(items[1]["body"]["type"], "follow");
    assert_eq!(items[1]["body"]["followed_by"], FOLLOWER_C);

    Ok(())
}
