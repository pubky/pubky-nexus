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
    const TEST_USER: &str = "qrqnkbwqt8rd6aya9zruro76gyc4qs77qzek7xc11axbdtbu6esy";
    const FOLLOWER_A: &str = "17775cing48kciy3enntgjwhw7yptxz6zw8k8czcddzk1yruqhcy";
    const FOLLOWER_B: &str = "xeeo9uagnox3obkwzud91bkjecgd5eaumbix4b13k8i7yorzjony";
    const FOLLOWER_C: &str = "hm8qoi5bjtrmx3kngiifycarjnnwosiohhbujax8fcyanzp3xa6y";

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
    const TEST_USER: &str = "rromxnw9y5cwww1h4b51rroxceinc3cefqpirptdqxb9qfchjxcy";
    const FOLLOWER_A: &str = "si53n9je46g3ygaoez39cnmhz4ocy7kscqatpietbzr6isahehio";
    const FOLLOWER_B: &str = "8swiuk8r7btkwryuawknd71w5ize5ca86jkr4kod1gcpwzsztmto";
    const FOLLOWER_C: &str = "myz6jgweja4jiyp3ckxqhkyqtfkofbn8rxo7496w1hwjj9459zzo";

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
    const TEST_USER: &str = "jwnbrorjegykfadh7tsiptzw4dppy1uj3hwp99xba9cibcib6zoy";
    const FOLLOWER_A: &str = "iemqqjrupf55i9eomobw33pwky9dwsdb8xgas3ymo9jk9o7b8roy";
    const FOLLOWER_B: &str = "eam6cr5oz4m4f8j6t8uamurr16eeybzh6g6u9sgsjc3ob6o3dfpy";
    const FOLLOWER_C: &str = "uasynstkt15esz5zoas7gjxbexc7kcbkxgphbzsyxmxyu3r4c3ey";
    const FOLLOWER_D: &str = "j4jf75ntxckgcmgtic9jy54h9xbmddnejj9wzeh3b66csnyf5uuy";
    const FOLLOWER_E: &str = "3854rbxu37rixyg5shcfpzsohzrwdfzmqxsn7jchogbw3mgskpmy";

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
    const TEST_USER: &str = "1ngp4fakazmu4apsw95w9mzmz4pfxm1jo359r881aci4o193siko";
    const FOLLOWER_A: &str = "meiif3edkaw4qdh79mjh9oh5dgsc71i685i5prq71oowgygtj9to";
    const FOLLOWER_B: &str = "sb6os3reiyxnxbwpd8sdj6uygco48fy6ycgxbxkdbsg9116awgto";
    const FOLLOWER_C: &str = "gobmmmrnq9umtufu3sdj6xhgnbwhgqcfhtngrsrgsdfe7iasd31o";

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
    const TEST_USER: &str = "7o8r4f96a9sri1efwdhbamnpntghiuz4gtgkazzf9txiu7oej7ry";
    const FOLLOWER_A: &str = "cmt1e5t9hkjka1giftphze3rmnnmxdk1bafg7ne71g4qxumb5esy";
    const FOLLOWER_B: &str = "tffi66yjexyy6guz6rpgx816rpneobifatau3j46fmfy3i5nx8co";
    const FOLLOWER_C: &str = "ow7e1wirjo5grk3peu8yzu4oked59ki6dbfhqd4mjzkpgwp67fny";

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
    const TEST_USER: &str = "h1z756b7jg8i595gh9iqnooj1dfkqyekt8yqyx9gxsnxk769966o";
    const FOLLOWER_A: &str = "iy9kc4m4hsoo775zp98q7bhef938769hwxdtsyqumjmi5prx5yao";
    const FOLLOWER_B: &str = "exkgz1yd4bj9xqwxftsdo7mrmmrm7c861c65zsugbk1csdinytxo";
    const FOLLOWER_C: &str = "pocb988o3ahy1k3achcq34q8shc13phnzfj9mkyq9nj3wnk5c46y";
    const FOLLOWER_D: &str = "waxkbh698e8kyhj5cgjbbe7z51rx6rmbeo4q87on1ddbnnrqoicy";
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
    const TEST_USER: &str = "a85p8hoso8pteigmc177oph83c6fp563o934szo4x8iouggey39o";
    const FOLLOWER_A: &str = "u1w6mahico6k6smxhrexzubkzmtg7so7c5xpwtd9mzkwktph1r8o";
    const FOLLOWER_B: &str = "b6uedjxmmns4rkkphu5i1aen9wyhcpt7nefrckp4cscf8w9q1tjo";
    const FOLLOWER_C: &str = "91h4zoeeonxyusxzkhub6knqdyg61rrijkmyjnf1aqsjngj5w8ey";
    const FOLLOWER_D: &str = "e7xe7b1gwcd485ce3srio9j5qz3opgrb3fpqeqmu64cznn5hqpeo";
    const FOLLOWER_E: &str = "jy94hp5doi16dok1faidk9m695nkcryucfnfpr9jt53epsmpyf6o";

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
