use crate::utils::{get_request, invalid_get_request, server::TestServiceServer};
use anyhow::Result;
use axum::http::StatusCode;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;

const NON_EXISTING_USER_ID: &str = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";

#[tokio_shared_rt::test(shared)]
async fn test_get_followers() -> Result<()> {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = get_request(&format!("/v0/user/{user_id}/followers")).await?;

    assert!(res.is_array());
    let followers: Vec<String> = res
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect();

    // List of specified IDs expected to follow the user
    let specified_follower_ids = vec![
        "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo".to_string(),
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy".to_string(),
        "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do".to_string(),
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo".to_string(),
        "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao".to_string(),
        "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty".to_string(),
        "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o".to_string(),
        "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy".to_string(),
        "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y".to_string(),
        "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y".to_string(),
    ];

    // Check if the user has the expected number of followers
    assert_eq!(
        followers.len(),
        specified_follower_ids.len(),
        "Unexpected number of followers"
    );

    // Check if all specified follower IDs are present in the followers list
    for id in &specified_follower_ids {
        assert!(followers.contains(id), "Missing follower ID: {id}");
    }

    // Test non-existing user
    invalid_get_request(
        &format!("/v0/user/{NON_EXISTING_USER_ID}/followers"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_following() -> Result<()> {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = get_request(&format!("/v0/user/{user_id}/following")).await?;

    assert!(res.is_array());
    let following: Vec<String> = res
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect();

    // List of specified IDs the user is expected to be following
    let specified_ids = vec![
        "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy".to_string(),
        "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y".to_string(),
        "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso".to_string(),
        "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y".to_string(),
        "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o".to_string(),
        "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty".to_string(),
        "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o".to_string(),
        "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno".to_string(),
        "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro".to_string(),
        "kg671gqu6akiyuzdsqgqtupftuhbuwfx5zh6tbygmexuw6b55s4y".to_string(),
        "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao".to_string(),
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo".to_string(),
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy".to_string(),
        "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do".to_string(),
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy".to_string(),
    ];

    // Check if the user is following the specified number of users
    assert_eq!(
        following.len(),
        specified_ids.len(),
        "Unexpected number of users followed"
    );

    // Check if all specified IDs are present in the following list
    for id in &specified_ids {
        assert!(following.contains(id), "Missing following ID: {id}");
    }

    // Test non-existing user
    invalid_get_request(
        &format!("/v0/user/{NON_EXISTING_USER_ID}/following"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_friends() -> Result<()> {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = get_request(&format!("/v0/user/{user_id}/friends")).await?;

    assert!(res.is_array());
    let following: Vec<String> = res
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect();

    // List of specified IDs the user is expected to be following
    let specified_ids = vec![
        "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y".to_string(),
        "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y".to_string(),
        "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o".to_string(),
        "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty".to_string(),
        "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao".to_string(),
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo".to_string(),
        "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do".to_string(),
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy".to_string(),
    ];

    // Check if the user friends the specified number of users
    assert_eq!(
        following.len(),
        specified_ids.len(),
        "Unexpected number of friends"
    );

    // Check if all specified IDs are present in the friend list
    for id in &specified_ids {
        assert!(following.contains(id), "Missing friend ID: {id}");
    }

    // Test non-existing user
    invalid_get_request(
        &format!("/v0/user/{NON_EXISTING_USER_ID}/friends"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

/// An existing user with no mutual follows must return `200 []`, not `404`.
///
/// Regression test: `Friends::get_by_id` used to collapse "user exists but has
/// no mutual friends" and "user does not exist" into the same `None`, so an
/// existing user with an empty intersection was reported as `404 User not found`
/// (inconsistent with `/followers` and `/following`, which return `200 []`).
#[tokio_shared_rt::test(shared)]
async fn test_get_friends_existing_user_without_mutuals() -> Result<()> {
    // This user exists in the fixture (it is in the main user's following list,
    // see `test_get_following`) and has both followers and followees, but no
    // reciprocal follow, so the mutual-friends intersection is empty.
    let user_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";

    // `get_request` asserts a 200 status, so a regression to 404 fails here.
    let res = get_request(&format!("/v0/user/{user_id}/friends")).await?;

    assert!(
        res.is_array(),
        "an existing user's friends response must be a JSON array"
    );
    assert_eq!(
        res.as_array().unwrap().len(),
        0,
        "an existing user with no mutual follows should return an empty list"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_follows_limit_cap() -> Result<()> {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    // limit=99999999 exceeds BoundedLimit<50, 200>::MAX → 400
    invalid_get_request(
        &format!("/v0/user/{user_id}/followers?limit=99999999"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    invalid_get_request(
        &format!("/v0/user/{user_id}/following?limit=99999999"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    invalid_get_request(
        &format!("/v0/user/{user_id}/friends?limit=99999999"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // skip=99999999 exceeds BoundedSkip<10_000>::MAX → 400
    invalid_get_request(
        &format!("/v0/user/{user_id}/followers?skip=99999999"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // limit=0 is rejected → 400
    invalid_get_request(
        &format!("/v0/user/{user_id}/followers?limit=0"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // limit=200 (at MAX) is accepted → 200
    get_request(&format!("/v0/user/{user_id}/followers?limit=200")).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_follows_skip_past_end() -> Result<()> {
    // The fixture user has 10 followers and follows 15 users. Skipping past the
    // end of either list must return 200 with an empty array, not 404.
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    let res = get_request(&format!("/v0/user/{user_id}/followers?skip=100")).await?;
    assert!(res.is_array());
    assert!(
        res.as_array().unwrap().is_empty(),
        "Expected empty followers page past the end"
    );

    let res = get_request(&format!("/v0/user/{user_id}/following?skip=100")).await?;
    assert!(res.is_array());
    assert!(
        res.as_array().unwrap().is_empty(),
        "Expected empty following page past the end"
    );

    // A nonexistent user still 404s, regardless of pagination
    invalid_get_request(
        &format!("/v0/user/{NON_EXISTING_USER_ID}/followers?skip=100"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    invalid_get_request(
        &format!("/v0/user/{NON_EXISTING_USER_ID}/following?skip=100"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_follows_pagination_slicing() -> Result<()> {
    // The fixture user has 10 followers and follows 15 users; skip/limit must
    // slice the list instead of being ignored. CI seeds a warm
    // cache, and the warm SSCAN path already slices, so each request below
    // deletes the cached set first to force the cold graph-fetch path.
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    // Ensure the server is running, so the Redis pool is initialized
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let followers_key = format!("Followers:{user_id}");
    let following_key = format!("Following:{user_id}");

    // Cold cache: limit smaller than the list returns exactly limit items
    let _: () = redis_conn.del(&followers_key).await?;
    let res = get_request(&format!("/v0/user/{user_id}/followers?limit=4")).await?;
    assert_eq!(
        res.as_array().unwrap().len(),
        4,
        "Expected followers limit to cap the page size on a cold cache"
    );

    // Cold cache: a mid-list skip returns 200 with the remainder instead of
    // 404ing (the aggregated single-row query used to drop the row on SKIP)
    let _: () = redis_conn.del(&followers_key).await?;
    let res = get_request(&format!("/v0/user/{user_id}/followers?skip=8&limit=4")).await?;
    assert_eq!(
        res.as_array().unwrap().len(),
        2,
        "Expected a cold mid-list skip to return the remainder"
    );

    // The cold request above cached the full list, so the warm SSCAN path
    // must serve a page of the same size
    let res = get_request(&format!("/v0/user/{user_id}/followers?skip=8&limit=4")).await?;
    assert_eq!(
        res.as_array().unwrap().len(),
        2,
        "Expected the warm cache to serve the same page size as the cold path"
    );

    // Same checks for the following list
    let _: () = redis_conn.del(&following_key).await?;
    let res = get_request(&format!("/v0/user/{user_id}/following?limit=6")).await?;
    assert_eq!(
        res.as_array().unwrap().len(),
        6,
        "Expected following limit to cap the page size on a cold cache"
    );

    let _: () = redis_conn.del(&following_key).await?;
    let res = get_request(&format!("/v0/user/{user_id}/following?skip=10&limit=10")).await?;
    assert_eq!(
        res.as_array().unwrap().len(),
        5,
        "Expected a cold mid-list skip to return the last following page"
    );

    let res = get_request(&format!("/v0/user/{user_id}/following?skip=10&limit=10")).await?;
    assert_eq!(
        res.as_array().unwrap().len(),
        5,
        "Expected the warm cache to serve the same page size as the cold path"
    );

    Ok(())
}
