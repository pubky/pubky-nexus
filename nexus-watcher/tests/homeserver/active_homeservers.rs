use crate::event_processor::utils::watcher::WatcherTest;
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

/// Active homeserver listing: orphan exclusion, sort order, and stability after user reassignment.
#[tokio_shared_rt::test(shared)]
async fn test_get_all_active_homeservers() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    // -- Orphan HSs (no users) must be excluded --
    let orphan1 = create_orphan_hs().await?;
    let orphan2 = create_orphan_hs().await?;

    // -- HS-A: 1 user --
    let hs_a = create_orphan_hs().await?;
    let kp_a1 = Keypair::random();
    let id_a1 = test
        .create_user(&kp_a1, &make_test_user("Watcher:ActiveHS:A1"))
        .await?;
    exec_single_row(queries::put::set_user_homeserver(&id_a1, &hs_a)).await?;

    // -- HS-B: 2 users --
    let hs_b = create_orphan_hs().await?;
    let kp_b1 = Keypair::random();
    let id_b1 = test
        .create_user(&kp_b1, &make_test_user("Watcher:ActiveHS:B1"))
        .await?;
    exec_single_row(queries::put::set_user_homeserver(&id_b1, &hs_b)).await?;

    let kp_b2 = Keypair::random();
    let id_b2 = test
        .create_user(&kp_b2, &make_test_user("Watcher:ActiveHS:B2"))
        .await?;
    exec_single_row(queries::put::set_user_homeserver(&id_b2, &hs_b)).await?;

    let hs_ids = Homeserver::get_all_active_from_graph().await?;

    // Orphans excluded
    assert!(
        !hs_ids.contains(&orphan1.to_string()),
        "orphan1 should be excluded"
    );
    assert!(
        !hs_ids.contains(&orphan2.to_string()),
        "orphan2 should be excluded"
    );

    // Both active HSs included, HS-B (2 users) before HS-A (1 user)
    let pos_a = hs_ids
        .iter()
        .position(|id| id == &hs_a.to_string())
        .expect("HS-A missing");
    let pos_b = hs_ids
        .iter()
        .position(|id| id == &hs_b.to_string())
        .expect("HS-B missing");
    assert!(pos_b < pos_a, "HS-B (2 users) should precede HS-A (1 user)");

    // -- Reassign one user from HS-B to HS-A; both HSs must stay active --
    exec_single_row(queries::put::set_user_homeserver(&id_b2, &hs_a)).await?;

    let hs_ids = Homeserver::get_all_active_from_graph().await?;
    assert!(
        hs_ids.contains(&hs_a.to_string()),
        "HS-A should still be active"
    );
    assert!(
        hs_ids.contains(&hs_b.to_string()),
        "HS-B should still be active after partial removal"
    );

    // Now HS-A has 2 users, HS-B has 1 — order should flip
    let pos_a = hs_ids
        .iter()
        .position(|id| id == &hs_a.to_string())
        .unwrap();
    let pos_b = hs_ids
        .iter()
        .position(|id| id == &hs_b.to_string())
        .unwrap();
    assert!(
        pos_a < pos_b,
        "HS-A (2 users) should now precede HS-B (1 user)"
    );

    // Cleanup
    test.cleanup_user(&kp_a1).await?;
    test.cleanup_user(&kp_b1).await?;
    test.cleanup_user(&kp_b2).await?;
    Ok(())
}
