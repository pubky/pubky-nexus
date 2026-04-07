use crate::event_processor::users::utils::find_user_details;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::RedisOps;
use nexus_common::models::event::EventProcessorError;
use nexus_common::models::user::{UserCounts, UserDetails, UserSearch};
use nexus_watcher::events::handlers::user;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyId};

/// Test that calling del() after a successful deletion returns Ok
/// and leaves no stale data behind.
#[tokio_shared_rt::test(shared)]
async fn test_user_del_idempotent() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a user with no relationships
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_user_del_idempotent".to_string()),
        image: None,
        links: None,
        name: "Watcher:IdempotentDel:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Delete through event processor (normal flow — everything succeeds)
    test.cleanup_user(&user_kp).await?;

    // Verify clean state after deletion
    let user_details_result = find_user_details(&user_id).await;
    assert!(
        user_details_result.is_err(),
        "User details should not be found in graph after deletion"
    );
    let user_counts = UserCounts::get_by_id(&user_id).await?;
    assert!(
        user_counts.is_none(),
        "User counts should not be found after deletion"
    );

    // Simulate retry: call del() directly — graph node is gone so expect SkipIndexing
    let user_pubky = PubkyId::try_from(user_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
    let result = user::del(user_pubky).await;
    assert!(
        matches!(result, Err(EventProcessorError::SkipIndexing)),
        "Retry after full deletion should return SkipIndexing, got: {:?}",
        result
    );

    // Verify state is still clean (no corruption from retry)
    let user_details_result = find_user_details(&user_id).await;
    assert!(
        user_details_result.is_err(),
        "User details should still not be found after retry"
    );
    let user_counts = UserCounts::get_by_id(&user_id).await?;
    assert!(
        user_counts.is_none(),
        "User counts should still not be found after retry"
    );

    Ok(())
}

/// Test graph-last recovery: simulates partial Redis cleanup failure where
/// graph node is still present. On retry, the handler should re-enter
/// CreatedOrDeleted, clean remaining stale Redis data, and delete graph last.
#[tokio_shared_rt::test(shared)]
async fn test_user_del_graph_last_recovery() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a user with no relationships
    let user_kp = Keypair::random();
    let username = "Watcher:GraphLastRecovery:User";
    let user = PubkyAppUser {
        bio: Some("test_user_del_graph_last_recovery".to_string()),
        image: None,
        links: None,
        name: username.to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Sanity: verify everything is set up
    let user_details = find_user_details(&user_id).await;
    assert!(user_details.is_ok(), "User should exist in graph");
    let user_counts = UserCounts::get_by_id(&user_id).await?;
    assert!(user_counts.is_some(), "User counts should exist in Redis");

    // Simulate partial failure of a previous del() attempt:
    // Step 1 succeeded: UserSearch cleaned
    UserSearch::delete(&user_id).await?;
    // Step 2 partially succeeded: UserDetails Redis DEL completed
    let user_id_str = user_id.as_str();
    let key_parts: &[&str] = &[user_id_str];
    UserDetails::remove_from_index_multiple_json(&[key_parts]).await?;
    // Step 2 partially failed: UserCounts::delete did NOT run (left stale)
    // Step 3 never ran: graph node still present (graph-last)

    // Verify the partial state: graph exists, UserCounts stale, UserDetails gone
    let user_details_graph = find_user_details(&user_id).await;
    assert!(
        user_details_graph.is_ok(),
        "Graph node should still exist (graph-last)"
    );
    let user_counts = UserCounts::get_by_id(&user_id).await?;
    assert!(
        user_counts.is_some(),
        "UserCounts should still be in Redis (simulated failure)"
    );

    // Retry: call del() directly — should recover and complete successfully
    let user_pubky = PubkyId::try_from(user_id.as_str()).map_err(|e| anyhow::anyhow!(e))?;
    user::del(user_pubky).await?;

    // Verify full cleanup after recovery
    let user_details_result = find_user_details(&user_id).await;
    assert!(
        user_details_result.is_err(),
        "Graph node should be deleted after recovery"
    );
    let user_counts = UserCounts::get_by_id(&user_id).await?;
    assert!(
        user_counts.is_none(),
        "UserCounts should be cleaned after recovery"
    );

    Ok(())
}
