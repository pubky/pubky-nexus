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
    const TEST_USER: &str = "ua5zuqm9jj6bm8tzxghz61uhn4xdq6p3x7f8afh6g7h5194kxocy";
    const FOLLOWER_A: &str = "8tzxghz61uhnwxdq6p3x7f8afc6g7h5194kxombhp53zf9ww9yso";
    const FOLLOWER_B: &str = "5kug4md7574d9sigpws85z5wedpkc5jcxzxzeoq4w3s1a9q9qtby";
    const FOLLOWER_C: &str = "xdq6p3x7f8afe6g7h5194kxokihp53zf9ww9yiua5zuqm9jj6bmo";

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
    const TEST_USER: &str = "nhksb85hqtsg1faicnxza7dcpemtkar9xt4ga4aznioj69dwptsy";
    const FOLLOWER_A: &str = "sigpws85z5w87pkc5jcxzxzex64w3s1a9q9qtyfijupft6767nyo";
    const FOLLOWER_B: &str = "kqnferu5bf1jgwhrkoj8snmr1tjaeiy1xcrsjfkuotkbr6ajc1my";
    const FOLLOWER_C: &str = "6g7h5194kxokthp53zf9ww9yi8a5zuqm9jj6bkztzxghz61uhnio";

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
    const TEST_USER: &str = "t93wxbm3u3qm5d9ue6nzu81hz489gthfxgxf3xhx6pdak6c6muyy";
    const FOLLOWER_A: &str = "faicnxza7dcprmtkar9xt4ga4cznioj69dwptibqfmyu768e5dko";
    const FOLLOWER_B: &str = "3ttdi6dagpkqxudn8mh8oc4w7dggrqzaxy3ij4qcce7xo6bukuiy";
    const FOLLOWER_C: &str = "pkc5jcxzxzexa4w3s1a9q9qo9iijupft6767b9ukug4md7574d9o";
    const FOLLOWER_D: &str = "bde1h45s3bgtnngtf3izp1npnerpnmumq5rr4rae4rzgs7sejwky";
    const FOLLOWER_E: &str = "whrkoj8snmr1pjaeiy1xcrsjr6uotkbr6ajc1kf8bnwnj7o13rwo";

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
    const TEST_USER: &str = "eiyndzuiminustkyr8x8kzkf8tnwyeq6qiqwkxkfeyo7h7k7ew9y";
    const FOLLOWER_A: &str = "hp53zf9ww9yiba5zuqm9jj6bk8tzxghz61uhnwzdq6p3x7f8afjo";
    const FOLLOWER_B: &str = "ogztkwmu6e6smypxniez8ht7c4y46fktqx3d43hbihkinh918iwy";
    const FOLLOWER_C: &str = "d9ue6nzu81hzw89gthfxgxf3xcx6pdak6c6m19y9h48oih3hzf6o";

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
    const TEST_USER: &str = "zaxyttd1oh4a9xo6bdn8fb3i1n9bhngrqkdumrp6darcehw8gsjy";
    const FOLLOWER_A: &str = "mtkar9xt4ga4eznioj69dwptwiqfmyu768e5dj1hksb85hqtsguo";
    const FOLLOWER_B: &str = "9kgxap5tdos5u6wc9o5zn8bpzm7e39bzqrqn5q94tu6dqhehfs6y";
    const FOLLOWER_C: &str = "udn8mh8oc4wh7ggrqzaxy3ij36cce7xo6bukuwraat49bhdgi8eo";

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
    const TEST_USER: &str = "g5769kuxsr16gpz576ig9cjfho5xz57kp6a1m3js9xz4w57trzuy";
    const FOLLOWER_A: &str = "4w3s1a9q9qo9tijupft6767b98kug4md7574d6sigpws85z5w87o";
    const FOLLOWER_B: &str = "qpiqg8mqeaxy4h5khcqshto6b33siaa7p3dbhd5upmtt451gdaey";
    const FOLLOWER_C: &str = "ngtf3izp1npnrrpnmumq5rr4rce4rzgs7sejwjytwjqpp5coue1o";
    const FOLLOWER_D: &str = "ua5zuqm9jj6bm8tzxghz61uhn4xdq6p3x7f8afh6g7h5194kxocy";
    const FOLLOWER_E: &str = "8tzxghz61uhnwxdq6p3x7f8afc6g7h5194kxombhp53zf9ww9yso";

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
    const TEST_USER: &str = "5kug4md7574d9sigpws85z5wedpkc5jcxzxzeoq4w3s1a9q9qtby";
    const FOLLOWER_A: &str = "xdq6p3x7f8afe6g7h5194kxokihp53zf9ww9yiua5zuqm9jj6bmo";
    const FOLLOWER_B: &str = "nhksb85hqtsg1faicnxza7dcpemtkar9xt4ga4aznioj69dwptsy";
    const FOLLOWER_C: &str = "sigpws85z5w87pkc5jcxzxzex64w3s1a9q9qtyfijupft6767nyo";
    const FOLLOWER_D: &str = "kqnferu5bf1jgwhrkoj8snmr1tjaeiy1xcrsjfkuotkbr6ajc1my";
    const FOLLOWER_E: &str = "6g7h5194kxokthp53zf9ww9yi8a5zuqm9jj6bkztzxghz61uhnio";

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
