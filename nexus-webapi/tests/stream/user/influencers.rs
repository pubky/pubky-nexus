use std::time::Duration;

use anyhow::Result;
use axum::http::StatusCode;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use tokio::time::sleep;
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

/// Recovery path for the AllTime influencers stream (issue #965): if the live
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

#[tokio_shared_rt::test(shared)]
async fn test_global_influencers_preview() -> Result<()> {
    let body = get_request("/v0/stream/users?source=influencers&preview=true").await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    assert!(!influencers.is_empty(), "Influencers should not be empty");

    // More info: nexus-common/src/models/user/influencers -> get_influencers()
    assert!(influencers.len() <= 3);
    debug!("Influencers length: {:?}", influencers.len());

    let first_influencer_ids: Vec<&str> = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect();

    // Sleep to ensure the second request gets a different timestamp_subsec_micros() value,
    // which determines the random skip offset for preview mode (see Influencers::get_influencers()).
    sleep(Duration::from_millis(5)).await;

    // Make a second request to verify preview returns different results
    let body = get_request("/v0/stream/users?source=influencers&preview=true").await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    assert!(!influencers.is_empty(), "Influencers should not be empty");
    assert!(influencers.len() <= 3);

    let second_influencer_ids: Vec<&str> = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect();

    assert!(first_influencer_ids != second_influencer_ids);

    Ok(())
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
