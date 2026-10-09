//! Recommended users over the fixture in docker/test-graph/mocks/recommended.cypher.
//! Each test reads a different observer's view of the same graph, so the cold
//! cache one test forces is never refilled by another.

use crate::utils::{
    get_request,
    recommended::{recommended_cache_key, D2, D3, D4, DELETED, FOLLOWED, HOP, OBS, SHORT},
    server::TestServiceServer,
};
use anyhow::Result;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;

/// The recommended ids of `user_id` as the graph query returns them, in a stable
/// order as the query has no ORDER BY. The cache is a set, which would hide a
/// duplicated id, so it is dropped first.
async fn get_sorted_recommended_ids_from_graph(user_id: &str) -> Result<Vec<String>> {
    // Ensure the server is running, so the Redis pool is initialized
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn.del(recommended_cache_key(user_id)).await?;

    let res = get_request(&format!(
        "/v0/stream/users/ids?source=recommended&user_id={user_id}&limit=20"
    ))
    .await?;

    let mut recommended_ids: Vec<String> = res
        .as_array()
        .expect("User id stream should be an array")
        .iter()
        .map(|id| id.as_str().expect("User id should be a string").to_string())
        .collect();
    recommended_ids.sort();
    Ok(recommended_ids)
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_recommended_result_set() -> Result<()> {
    let recommended_ids = get_sorted_recommended_ids_from_graph(OBS).await?;

    assert!(
        !recommended_ids.contains(&FOLLOWED.to_string()),
        "A directly followed user should not be recommended, even if reachable at depth 2"
    );
    assert!(
        !recommended_ids.contains(&HOP.to_string()),
        "A directly followed user should not be recommended"
    );
    assert!(
        !recommended_ids.contains(&OBS.to_string()),
        "A follow cycle should not recommend the user to themselves"
    );
    assert!(
        !recommended_ids.contains(&SHORT.to_string()),
        "A user one post short of the threshold should not be recommended"
    );
    assert!(
        !recommended_ids.contains(&DELETED.to_string()),
        "A deleted user should not be recommended"
    );
    assert!(
        !recommended_ids.contains(&D4.to_string()),
        "A user at depth 4 should not be recommended"
    );

    // D2 is on the post threshold and reached over two paths: listed exactly once
    let mut expected_ids = vec![D2.to_string(), D3.to_string()];
    expected_ids.sort();
    assert_eq!(
        recommended_ids, expected_ids,
        "Only the active users at depth 2 and 3 should be recommended, each once"
    );

    Ok(())
}

/// HOP follows everyone OBS reaches at depth 2, which moves the whole graph one
/// step closer: D4 comes into range at depth 3 and the depth 2 users of OBS are
/// directly followed.
#[tokio_shared_rt::test(shared)]
async fn test_stream_recommended_depth_is_relative_to_the_user() -> Result<()> {
    let recommended_ids = get_sorted_recommended_ids_from_graph(HOP).await?;

    let mut expected_ids = vec![D3.to_string(), D4.to_string()];
    expected_ids.sort();
    assert_eq!(
        recommended_ids, expected_ids,
        "Only the users at depth 2 and 3 of HOP should be recommended"
    );

    Ok(())
}
