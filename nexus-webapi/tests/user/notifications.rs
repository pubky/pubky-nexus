use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::{
    db::RedisOps,
    models::notification::{Notification, NotificationBody},
};
use serde_json::Value;

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

/// Verifies that limit=0 is rejected with 400 (must be at least 1).
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_with_limit_zero() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "rromxnw9y5cwww1h4b51rroxceinc3cefqpirptdqxb9qfchjxcy";

    invalid_get_request(
        &format!("/v0/user/{TEST_USER}/notifications?limit=0"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

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

/// Pins the `post_kind` wire contract: a new-format `TagPost` serializes the
/// kind as a lowercase string, and an old-format row (no `post_kind`) reads
/// back as `unknown` via the serde default.
#[tokio_shared_rt::test(shared)]
async fn test_notification_post_kind_wire_and_backcompat() -> Result<()> {
    env_init().await;
    // Valid z-base32 IDs borrowed from other tests' FOLLOWER constants; those
    // ids are only ever used as `followed_by` values, never as recipients, so
    // this recipient's notification list starts empty.
    const TEST_USER: &str = "ow7e1wirjo5grk3peu8yzu4oked59ki6dbfhqd4mjzkpgwp67fny";
    const TAGGER: &str = "17775cing48kciy3enntgjwhw7yptxz6zw8k8czcddzk1yruqhcy";

    // New-format: a collection tag carries post_kind = Collection.
    let new_body = NotificationBody::TagPost {
        tagged_by: TAGGER.to_string(),
        tag_label: "curated".to_string(),
        post_uri: format!("pubky://{TEST_USER}/pub/pubky.app/posts/0000000000000"),
        post_kind: nexus_common::models::post::PostKind::Collection,
    };
    Notification::put_index_sorted_set(
        &["Notification", TEST_USER],
        &[(2000.0, serde_json::to_string(&new_body).unwrap().as_str())],
        None,
        None,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    // Old-format: a pre-field row stored without `post_kind`.
    let old_json = format!(
        r#"{{"type":"tag_post","tagged_by":"{TAGGER}","tag_label":"legacy","post_uri":"pubky://{TEST_USER}/pub/pubky.app/posts/0000000000001"}}"#
    );
    Notification::put_index_sorted_set(
        &["Notification", TEST_USER],
        &[(1000.0, old_json.as_str())],
        None,
        None,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    let res = get_request(&format!("/v0/user/{TEST_USER}/notifications")).await?;
    let items = res.as_array().unwrap();
    assert_eq!(items.len(), 2, "Expected both seeded notifications");

    // Newest first: the new-format collection tag.
    assert_eq!(items[0]["body"]["type"], "tag_post");
    assert_eq!(items[0]["body"]["post_kind"], "collection");

    // The old-format row still deserializes and defaults post_kind to unknown.
    assert_eq!(items[1]["body"]["type"], "tag_post");
    assert_eq!(items[1]["body"]["post_kind"], "unknown");

    Ok(())
}

/// Inserts one raw notification body with an explicit timestamp.
async fn seed_body(recipient_id: &str, body: &NotificationBody, timestamp: i64) -> Result<()> {
    let json = serde_json::to_string(body).unwrap();
    Notification::put_index_sorted_set(
        &["Notification", recipient_id],
        &[(timestamp as f64, json.as_str())],
        None,
        None,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e}"))
}

fn reply_body(replier: &str, n: u32) -> NotificationBody {
    NotificationBody::Reply {
        replied_by: replier.to_string(),
        parent_post_uri: format!("pubky://{replier}/pub/pubky.app/posts/P{n:012}"),
        reply_uri: format!("pubky://{replier}/pub/pubky.app/posts/R{n:012}"),
        post_kind: nexus_common::models::post::PostKind::Short,
    }
}

fn mention_body(mentioner: &str, n: u32) -> NotificationBody {
    NotificationBody::Mention {
        mentioned_by: mentioner.to_string(),
        post_uri: format!("pubky://{mentioner}/pub/pubky.app/posts/M{n:012}"),
        post_kind: nexus_common::models::post::PostKind::Short,
    }
}

/// Seeds 30 follows on top of 3 older replies and 1 mention, so an unfiltered
/// first page holds only follows. `types=reply` must reach past them and return
/// the 3 replies newest-first; `types=reply,mention` returns 4 items.
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_filtered_by_type() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "tffi66yjexyy6guz6rpgx816rpneobifatau3j46fmfy3i5nx8co";
    const OTHER: &str = "17775cing48kciy3enntgjwhw7yptxz6zw8k8czcddzk1yruqhcy";

    seed_body(TEST_USER, &reply_body(OTHER, 1), 1000).await?;
    seed_body(TEST_USER, &mention_body(OTHER, 1), 1500).await?;
    seed_body(TEST_USER, &reply_body(OTHER, 2), 2000).await?;
    seed_body(TEST_USER, &reply_body(OTHER, 3), 3000).await?;
    for i in 0..30u32 {
        let follower = format!("{OTHER}-{i}");
        seed_follow(TEST_USER, &follower, 10_000 + i as i64).await?;
    }

    let page = get_request(&format!("/v0/user/{TEST_USER}/notifications?limit=20")).await?;
    let page = page.as_array().unwrap();
    assert!(page.iter().all(|n| n["body"]["type"] == "follow"));

    let replies = get_request(&format!("/v0/user/{TEST_USER}/notifications?types=reply")).await?;
    let replies = replies.as_array().unwrap();
    assert_eq!(replies.len(), 3, "expected only the 3 replies: {replies:?}");
    assert!(replies.iter().all(|n| n["body"]["type"] == "reply"));
    let ts: Vec<i64> = replies
        .iter()
        .map(|n| n["timestamp"].as_i64().unwrap())
        .collect();
    assert_eq!(ts, vec![3000, 2000, 1000]);

    // skip and limit count matching notifications only.
    let paged = get_request(&format!(
        "/v0/user/{TEST_USER}/notifications?types=reply&skip=1&limit=1"
    ))
    .await?;
    let paged = paged.as_array().unwrap();
    assert_eq!(paged.len(), 1);
    assert_eq!(paged[0]["timestamp"], 2000_i64);

    // `start` keeps working as the cursor for the next filtered page.
    let older = get_request(&format!(
        "/v0/user/{TEST_USER}/notifications?types=reply&start=1999"
    ))
    .await?;
    let older = older.as_array().unwrap();
    assert_eq!(older.len(), 1);
    assert_eq!(older[0]["timestamp"], 1000_i64);

    let two = get_request(&format!(
        "/v0/user/{TEST_USER}/notifications?types=reply,mention"
    ))
    .await?;
    let two = two.as_array().unwrap();
    assert_eq!(two.len(), 4);
    assert_eq!(two[2]["body"]["type"], "mention");

    Ok(())
}

/// An unknown type name and an empty list are rejected with 400.
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_invalid_type_filter() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "tffi66yjexyy6guz6rpgx816rpneobifatau3j46fmfy3i5nx8co";

    invalid_get_request(
        &format!("/v0/user/{TEST_USER}/notifications?types=bogus"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    invalid_get_request(
        &format!("/v0/user/{TEST_USER}/notifications?types=,"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    Ok(())
}

/// GET that also returns one response header, for the filtered-scan cursor.
async fn get_with_header(endpoint: &str, header: &str) -> Result<(Value, Option<String>)> {
    let url = crate::utils::host_url().await;
    let client = httpc_test::new_client("")?;
    let res = client.do_get(&format!("{url}{endpoint}")).await?;
    assert_eq!(res.status(), 200, "Expected HTTP status 200 OK");
    Ok((res.json_body()?, res.header(header)))
}

/// Seeds one reply below 10,001 follows. The filtered scan stops at the cap with
/// no match: the page is empty but `X-Next-Start` says where to resume, and
/// resuming from it finds the reply with no header (the history is exhausted).
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_filtered_scan_cap_returns_cursor() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "e7xe7b1gwcd485ce3srio9j5qz3opgrb3fpqeqmu64cznn5hqpeo";
    const OTHER: &str = "17775cing48kciy3enntgjwhw7yptxz6zw8k8czcddzk1yruqhcy";

    seed_body(TEST_USER, &reply_body(OTHER, 1), 1_000).await?;
    let follows: Vec<(f64, String)> = (0..10_001u32)
        .map(|i| {
            let body = NotificationBody::Follow {
                followed_by: format!("{OTHER}-{i}"),
            };
            (100_000.0 + i as f64, serde_json::to_string(&body).unwrap())
        })
        .collect();
    let refs: Vec<(f64, &str)> = follows.iter().map(|(s, b)| (*s, b.as_str())).collect();
    Notification::put_index_sorted_set(&["Notification", TEST_USER], &refs, None, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let (first, cursor) = get_with_header(
        &format!("/v0/user/{TEST_USER}/notifications?types=reply"),
        "x-next-start",
    )
    .await?;
    assert_eq!(first.as_array().unwrap().len(), 0);
    let cursor = cursor.expect("a capped scan must return X-Next-Start");
    // 10,000 newest follows scanned (100_001..=110_000); resume just below them.
    assert_eq!(cursor, "100000");

    let (second, cursor) = get_with_header(
        &format!("/v0/user/{TEST_USER}/notifications?types=reply&start={cursor}"),
        "x-next-start",
    )
    .await?;
    let second = second.as_array().unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0]["timestamp"], 1_000_i64);
    assert!(
        cursor.is_none(),
        "an exhausted scan must not return a cursor"
    );

    Ok(())
}

/// 520 follows share one timestamp across the 500-row chunk boundary. The
/// chunked scan must step past the ties without skipping or repeating a row.
#[tokio_shared_rt::test(shared)]
async fn test_get_notifications_filtered_ties_across_chunks() -> Result<()> {
    env_init().await;
    const TEST_USER: &str = "jy94hp5doi16dok1faidk9m695nkcryucfnfpr9jt53epsmpyf6o";
    const OTHER: &str = "17775cing48kciy3enntgjwhw7yptxz6zw8k8czcddzk1yruqhcy";

    let mut rows: Vec<(f64, String)> = (0..520u32)
        .map(|i| {
            let body = NotificationBody::Follow {
                followed_by: format!("{OTHER}-{i}"),
            };
            (5_000.0, serde_json::to_string(&body).unwrap())
        })
        .collect();
    for n in 0..3u32 {
        rows.push((
            5_000.0,
            serde_json::to_string(&reply_body(OTHER, n)).unwrap(),
        ));
    }
    rows.push((
        4_000.0,
        serde_json::to_string(&reply_body(OTHER, 9)).unwrap(),
    ));
    let refs: Vec<(f64, &str)> = rows.iter().map(|(s, b)| (*s, b.as_str())).collect();
    Notification::put_index_sorted_set(&["Notification", TEST_USER], &refs, None, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let res = get_request(&format!("/v0/user/{TEST_USER}/notifications?types=reply")).await?;
    let items = res.as_array().unwrap();
    let uris: std::collections::HashSet<String> = items
        .iter()
        .map(|n| n["body"]["reply_uri"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(items.len(), 4, "each reply exactly once: {items:?}");
    assert_eq!(uris.len(), 4);
    assert_eq!(items[3]["timestamp"], 4_000_i64);

    Ok(())
}
