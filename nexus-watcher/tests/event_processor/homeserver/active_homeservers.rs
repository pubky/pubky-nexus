use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::exec_single_row;
use nexus_common::db::queries;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyId};

/// Helper: create a PubkyAppUser with a given name.
fn make_test_user(name: &str) -> PubkyAppUser {
    PubkyAppUser {
        bio: Some(format!("bio-{name}")),
        image: None,
        links: None,
        name: name.to_string(),
        status: None,
    }
}

/// Helper: create an orphan homeserver (no users) in the graph, return its PubkyId.
async fn create_orphan_hs() -> Result<PubkyId, DynError> {
    let keys = Keypair::random();
    let id = PubkyId::try_from(&keys.public_key().to_z32())?;
    let hs = Homeserver::new(id.clone());
    hs.put_to_graph().await?;
    Ok(id)
}

#[tokio_shared_rt::test(shared)]
async fn test_get_all_homeservers_excludes_orphan() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    // Create an orphan homeserver (no users hosted on it)
    let orphan_id = create_orphan_hs().await?;

    // Create a user via WatcherTest, which persists the user in the graph
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &make_test_user("Watcher:AllHS:User"))
        .await?;

    // Link the user to the test homeserver via HOSTED_BY
    let default_id = test.homeserver_id.clone();
    let link_query = queries::put::set_user_homeserver(&user_id, &default_id);
    exec_single_row(link_query).await?;

    // Query all active homeservers
    let hs_ids = Homeserver::get_all_active_from_graph().await?;

    // The active homeserver must be present
    assert!(
        hs_ids.contains(&default_id.to_string()),
        "Active HS should be included"
    );

    // The orphan homeserver must NOT be present (no active users)
    assert!(
        !hs_ids.contains(&orphan_id.to_string()),
        "Orphan HS should be excluded"
    );

    // Cleanup
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// Multiple orphan homeservers must all be excluded from the active list.
#[tokio_shared_rt::test(shared)]
async fn test_get_all_homeservers_excludes_multiple_orphans() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    // Create several orphan homeservers
    let orphan_ids: Vec<PubkyId> = {
        let mut ids = Vec::new();
        for _ in 0..3 {
            ids.push(create_orphan_hs().await?);
        }
        ids
    };

    // Create one active user on the default homeserver so the query returns at least one HS
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &make_test_user("MultiOrphan:User"))
        .await?;
    let default_id = test.homeserver_id.clone();
    exec_single_row(queries::put::set_user_homeserver(&user_id, &default_id)).await?;

    let hs_ids = Homeserver::get_all_active_from_graph().await?;

    assert!(
        hs_ids.contains(&default_id.to_string()),
        "Active HS should be included"
    );
    for orphan_id in &orphan_ids {
        assert!(
            !hs_ids.contains(&orphan_id.to_string()),
            "Orphan HS {orphan_id} should be excluded"
        );
    }

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Multiple active homeservers with different user counts are all returned,
/// and sorted by user count descending.
#[tokio_shared_rt::test(shared)]
async fn test_get_all_homeservers_returns_multiple_active_sorted() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    // --- HS-A: 1 user ---
    let hs_a_id = create_orphan_hs().await?;
    let user_a1_kp = Keypair::random();
    let user_a1_id = test
        .create_user(&user_a1_kp, &make_test_user("SortA1"))
        .await?;
    exec_single_row(queries::put::set_user_homeserver(&user_a1_id, &hs_a_id)).await?;

    // --- HS-B: 2 users ---
    let hs_b_id = create_orphan_hs().await?;
    let user_b1_kp = Keypair::random();
    let user_b1_id = test
        .create_user(&user_b1_kp, &make_test_user("SortB1"))
        .await?;
    exec_single_row(queries::put::set_user_homeserver(&user_b1_id, &hs_b_id)).await?;

    let user_b2_kp = Keypair::random();
    let user_b2_id = test
        .create_user(&user_b2_kp, &make_test_user("SortB2"))
        .await?;
    exec_single_row(queries::put::set_user_homeserver(&user_b2_id, &hs_b_id)).await?;

    let hs_ids = Homeserver::get_all_active_from_graph().await?;

    // Both must appear
    assert!(
        hs_ids.contains(&hs_a_id.to_string()),
        "HS-A (1 user) should be included"
    );
    assert!(
        hs_ids.contains(&hs_b_id.to_string()),
        "HS-B (2 users) should be included"
    );

    // HS-B (2 users) must appear before HS-A (1 user) because the query sorts DESC
    let pos_a = hs_ids
        .iter()
        .position(|id| id == &hs_a_id.to_string())
        .unwrap();
    let pos_b = hs_ids
        .iter()
        .position(|id| id == &hs_b_id.to_string())
        .unwrap();
    assert!(
        pos_b < pos_a,
        "HS-B (2 users) should be listed before HS-A (1 user)"
    );

    // Cleanup
    test.cleanup_user(&user_a1_kp).await?;
    test.cleanup_user(&user_b1_kp).await?;
    test.cleanup_user(&user_b2_kp).await?;

    Ok(())
}

/// A homeserver with multiple users stays active after one user is removed.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_stays_active_after_partial_user_removal() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    let hs_id_1 = create_orphan_hs().await?;
    let hs_id_2 = create_orphan_hs().await?;

    // Create two users on the same HS
    let user1_kp = Keypair::random();
    let user1_id = test
        .create_user(&user1_kp, &make_test_user("Partial:User1"))
        .await?;
    exec_single_row(queries::put::set_user_homeserver(&user1_id, &hs_id_1)).await?;

    let user2_kp = Keypair::random();
    let user2_id = test
        .create_user(&user2_kp, &make_test_user("Partial:User2"))
        .await?;
    exec_single_row(queries::put::set_user_homeserver(&user2_id, &hs_id_1)).await?;

    // Both users present — HS 1 is active
    let hs_ids = Homeserver::get_all_active_from_graph().await?;
    assert!(
        hs_ids.contains(&hs_id_1.to_string()),
        "HS 1 with two users should be active"
    );

    // User 2 switches to HS 2
    exec_single_row(queries::put::set_user_homeserver(&user2_id, &hs_id_2)).await?;

    // HS 1 should still be active with the remaining user
    let hs_ids_after = Homeserver::get_all_active_from_graph().await?;
    assert!(
        hs_ids_after.contains(&hs_id_1.to_string()),
        "HS 1 should stay active after removing one of two users"
    );

    // Cleanup
    test.cleanup_user(&user1_kp).await?;
    test.cleanup_user(&user2_kp).await?;

    Ok(())
}
