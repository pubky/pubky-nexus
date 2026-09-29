use anyhow::Result;
use axum::http::StatusCode;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::models::user::GLOBAL_INFLUENCERS_CACHE_SIZE;
use tracing::debug;

use crate::{
    tags::hot::USER_1,
    utils::{get_request, invalid_get_request, server::TestServiceServer},
};

#[tokio_shared_rt::test(shared)]
async fn test_global_influencers() -> Result<()> {
    let body = get_request("/v0/stream/users?source=influencers").await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    assert!(!influencers.is_empty(), "Influencers should not be empty");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy",
        "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o",
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo",
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy",
    ];

    assert!(influencer_ids == expected_user_ids);

    Ok(())
}

/// Recovery path for the AllTime influencers stream: if the live
/// `Sorted:Users:Influencers` set is lost (e.g. after Redis data loss), the next request
/// must fall back to the graph, reseed that same key, and still return results.
#[tokio_shared_rt::test(shared)]
async fn test_global_influencers_alltime_cache_recovery() -> Result<()> {
    // Ensure the server is running, so the Redis pool is initialized
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;

    let sorted_set_key = "Sorted:Users:Influencers";

    // Snapshot the live sorted set (members + scores) so it can be restored afterwards.
    // The graph-derived fallback scores only approximate the counts-derived ones, and
    // sibling tests assert on the exact original ordering.
    let snapshot: Vec<(String, f64)> = redis_conn.zrange_withscores(sorted_set_key, 0, -1).await?;

    // Simulate data loss of the live AllTime influencers sorted set
    let _: () = redis_conn.del(sorted_set_key).await?;

    // The timeframe defaults to AllTime, so this request exercises the graph fallback.
    // Defer the `?` until after the restore below so a failed request cannot leave the
    // shared sorted set poisoned for sibling tests.
    let body_result = get_request("/v0/stream/users?source=influencers").await;

    // The fallback must have reseeded the same key the AllTime read path uses
    let reseeded: bool = redis_conn.exists(sorted_set_key).await?;

    // Restore the original sorted set exactly (DEL then ZADD of the snapshot) before
    // asserting, so any later test observes the original counts-derived scores.
    let _: () = redis_conn.del(sorted_set_key).await?;
    if !snapshot.is_empty() {
        let items: Vec<(f64, &str)> = snapshot
            .iter()
            .map(|(member, score)| (*score, member.as_str()))
            .collect();
        let _: () = redis_conn.zadd_multiple(sorted_set_key, &items).await?;
    }

    let body = body_result?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");
    assert!(
        !influencers.is_empty(),
        "Influencers should be repopulated from the graph after the sorted set is lost"
    );

    assert!(
        reseeded,
        "The graph fallback should rewrite the Sorted:Users:Influencers sorted set"
    );

    Ok(())
}

/// How many preview requests the test samples before giving up on seeing two distinct
/// windows. A fixed offset repeats on every request and fails; a clock-derived offset
/// would have to repeat 20 times in a row to fail, which is about 98^-19.
const PREVIEW_PROBES: usize = 20;

/// The users stream caps a page at 20 entries: `BoundedPagination<10_000, 5, 20>` on the
/// route in `nexus-webapi/src/routes/v0/stream/users.rs`.
const USERS_PAGE_MAX: usize = 20;

#[tokio_shared_rt::test(shared)]
async fn test_global_influencers_preview() -> Result<()> {
    let ranking_ids = global_influencers_ranking().await?;

    // The offset a preview starts at comes from the clock (`preview_skip` in
    // nexus-common, unit tested there), so no single response can be asserted to be
    // random: two clock samples collide whenever they are a multiple of 98 us apart and
    // then return the same window. What is deterministic is the shape of one response,
    // and that the offset is not a constant. So sample a bounded number of previews,
    // require each one to be a contiguous window of the ranking, and require at least
    // two different windows.
    let mut windows: Vec<Vec<String>> = Vec::new();

    for _ in 0..PREVIEW_PROBES {
        let preview_ids = preview_window_ids().await?;
        assert_contiguous_window(&ranking_ids, &preview_ids);

        if !windows.contains(&preview_ids) {
            windows.push(preview_ids);
        }

        if windows.len() > 1 {
            break;
        }
    }

    assert!(
        windows.len() > 1,
        "preview returned the same window on all {PREVIEW_PROBES} requests: the offset \
         looks constant rather than derived from the clock"
    );

    Ok(())
}

/// The ids of one preview response, in order.
async fn preview_window_ids() -> Result<Vec<String>> {
    let body = get_request("/v0/stream/users?source=influencers&preview=true").await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    assert!(!influencers.is_empty(), "Influencers should not be empty");

    // More info: nexus-common/src/models/user/influencers -> get_influencers()
    assert!(influencers.len() <= 3);
    debug!("Influencers length: {:?}", influencers.len());

    Ok(influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap().to_string())
        .collect())
}

/// A preview is a contiguous window of the global ranking: the same entries, in the same
/// order, with nothing skipped in between.
fn assert_contiguous_window(ranking_ids: &[String], preview_ids: &[String]) {
    let start = ranking_ids
        .iter()
        .position(|id| *id == preview_ids[0])
        .expect("The first preview influencer must appear in the global ranking");

    assert_eq!(
        ranking_ids.get(start..start + preview_ids.len()),
        Some(preview_ids),
        "A preview must be a contiguous window of the global ranking"
    );
}

/// The global influencers ranking as the API serves it: the cached top
/// `GLOBAL_INFLUENCERS_CACHE_SIZE` entries, read as pages of at most `USERS_PAGE_MAX`.
/// A preview window can start at the last offset of the cache, so the whole cache is read.
async fn global_influencers_ranking() -> Result<Vec<String>> {
    let mut ranking = Vec::new();

    for skip in (0..GLOBAL_INFLUENCERS_CACHE_SIZE).step_by(USERS_PAGE_MAX) {
        let body = get_request(&format!(
            "/v0/stream/users?source=influencers&skip={skip}&limit={USERS_PAGE_MAX}"
        ))
        .await?;
        let page = body
            .as_array()
            .expect("Stream influencers should be an array");
        assert!(
            !page.is_empty(),
            "the mock fixture must seed at least GLOBAL_INFLUENCERS_CACHE_SIZE \
             ({GLOBAL_INFLUENCERS_CACHE_SIZE}) ranked users, but the page at skip {skip} \
             was empty"
        );

        ranking.extend(
            page.iter()
                .map(|f| f["details"]["id"].as_str().unwrap().to_string()),
        );
    }

    Ok(ranking)
}

#[tokio_shared_rt::test(shared)]
async fn test_global_influencers_skip_limit() -> Result<()> {
    let body = get_request("/v0/stream/users?source=influencers&skip=3&limit=3").await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    // assert limit
    assert_eq!(influencers.len(), 3);

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy",
        "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so",
    ];

    assert!(influencer_ids == expected_user_ids);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_global_influencers_with_today_timeframe() -> Result<()> {
    let body = get_request("/v0/stream/users?source=influencers&timeframe=today&limit=4").await?;

    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco",
        "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy",
        "otn147ixg3i4sorqupuzptnx9gtiku4y77i8fdo35m7yug1d8zio",
        "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy",
    ];

    // Verify that each expected user ID is present in the response
    for id in &expected_user_ids {
        let exists = influencer_ids.clone().into_iter().any(|item| item == *id);
        assert!(exists, "Expected user ID not found: {id}");
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_global_influencers_with_this_month_timeframe() -> Result<()> {
    let body =
        get_request("/v0/stream/users?source=influencers&timeframe=this_month&limit=5").await?;

    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco",
        "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy",
        "otn147ixg3i4sorqupuzptnx9gtiku4y77i8fdo35m7yug1d8zio",
        "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy",
        "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo",
    ];

    // Verify that each expected user ID is present in the response
    for id in &expected_user_ids {
        let exists = influencer_ids.clone().into_iter().any(|item| item == *id);
        assert!(exists, "Expected user ID not found: {id}");
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_influencers_by_reach_no_user_id() -> Result<()> {
    let endpoint =
        "/v0/stream/users?source=influencers&timeframe=this_month&limit=3&reach=following";

    invalid_get_request(endpoint, StatusCode::BAD_REQUEST).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_influencers_by_following_reach() -> Result<()> {
    let endpoint = &format!("/v0/stream/users?source=influencers&timeframe=this_month&limit=3&user_id={USER_1}&reach=following");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "r91hi8kc3x6761gwfiigr7yn6nca1z47wm6jadhw1jbx1co93r9y",
        "6xejaazm58f5dca3aj6o4is3k55wxy86hyxtd1pu5h897cfq76yy",
        "qumq6fady4bmw4w5tpsrj1tg36g3qo4tcfedga9p4bg4so4ikyzy",
    ];
    assert!(influencer_ids == expected_user_ids);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_influencers_by_followers_reach() -> Result<()> {
    let endpoint = &format!("/v0/stream/users?source=influencers&timeframe=this_month&limit=3&user_id={USER_1}&reach=followers");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let influencers = body.as_array().expect("Post stream should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec!["tkpeqpx3ywoawiw6q8e6kuo9o3egr7fnhx83rudznbrrmqgdmomo"];
    assert!(influencer_ids == expected_user_ids);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_influencers_by_friends_reach() -> Result<()> {
    let endpoint = &format!(
        "/v0/stream/users?source=influencers&timeframe=this_month&limit=3&user_id={USER_1}&reach=friends"
    );

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let influencers = body.as_array().expect("Post stream should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec!["tkpeqpx3ywoawiw6q8e6kuo9o3egr7fnhx83rudznbrrmqgdmomo"];
    assert!(influencer_ids == expected_user_ids);

    Ok(())
}

/// Ranged global influencers come from a 100-entry cache, so a skip past it can never
/// fill a page. It is rejected up front instead of being served as a cache miss.
#[tokio_shared_rt::test(shared)]
async fn test_global_influencers_rejects_skip_past_cache() -> Result<()> {
    let body = invalid_get_request(
        "/v0/stream/users?source=influencers&timeframe=today&skip=101",
        StatusCode::BAD_REQUEST,
    )
    .await?;
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("skip must be at most 100"),
        "unexpected error payload: {body}"
    );

    // Preview ignores skip, so the cap does not apply to it.
    let body =
        get_request("/v0/stream/users?source=influencers&timeframe=today&skip=101&preview=true")
            .await?;
    assert!(body.is_array());

    // Only ranged global influencers are capped: the all-time index is not size-bound,
    // and reach-scoped queries go to the graph with their own pagination.
    let body = get_request("/v0/stream/users?source=influencers&skip=101").await?;
    assert!(body.is_array());
    let body = get_request(&format!(
        "/v0/stream/users?source=influencers&timeframe=this_month&skip=101&user_id={USER_1}&reach=following"
    ))
    .await?;
    assert!(body.is_array());

    Ok(())
}

/// An in-range skip past the last cached entry is an empty page, not a cache miss. A miss
/// would refetch from the graph and rewrite the key, which re-arms its TTL; so after the
/// TTL is pinned low, an empty-window read must leave it low.
#[tokio_shared_rt::test(shared)]
async fn test_global_influencers_skip_to_cache_end_is_empty_page() -> Result<()> {
    // Ensure the server is running, so the Redis pool is initialized
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let key = "Cache:Influencers:Today";

    // Warm the cache so the key exists, then pin its TTL well below the 1h cache period.
    get_request("/v0/stream/users?source=influencers&timeframe=today&limit=1").await?;
    let pinned_ttl = 600;
    let _: () = redis_conn.expire(key, pinned_ttl).await?;

    let body = get_request("/v0/stream/users?source=influencers&timeframe=today&skip=100").await?;
    assert_eq!(
        body.as_array().map(Vec::len),
        Some(0),
        "skip at the cache size must yield an empty page, got: {body}"
    );

    let ttl: i64 = redis_conn.ttl(key).await?;
    assert!(
        0 < ttl && ttl <= pinned_ttl,
        "an empty window must not be treated as a cache miss and rewrite the key, ttl went to {ttl}"
    );

    Ok(())
}
