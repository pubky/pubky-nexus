use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::exec_single_row;
use nexus_common::db::queries;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyId};

#[tokio_shared_rt::test(shared)]
async fn test_get_all_homeservers_includes_orphan_and_active() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    // Create an orphan homeserver (no users hosted on it)
    let orphan_keys = Keypair::random();
    let orphan_id = PubkyId::try_from(&orphan_keys.public_key().to_z32())?;
    let orphan_hs = Homeserver::new(orphan_id.clone());
    orphan_hs.put_to_graph().await?;

    // Create a user via WatcherTest, which persists the user in the graph
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_get_all_homeservers".to_string()),
        image: None,
        links: None,
        name: "Watcher:AllHS:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Link the user to the test homeserver via HOSTED_BY
    let default_id = test.homeserver_id.clone();
    let link_query = queries::put::set_user_homeserver(&user_id, &default_id);
    exec_single_row(link_query).await?;

    // Query all homeservers
    let hs_entries = Homeserver::get_all_from_graph().await?;
    let hs_ids: Vec<&str> = hs_entries.iter().map(|e| e.id.as_str()).collect();

    // Both the active and the orphan homeserver must be present
    assert!(hs_ids.contains(&default_id.as_str()), "Active HS should be included");
    assert!(
        hs_ids.contains(&orphan_id.as_str()),
        "Orphan HS should also be included"
    );

    // The active homeserver must have at least 1 active user
    let active_entry = hs_entries
        .iter()
        .find(|e| e.id == default_id.as_str())
        .expect("Active HS entry should be present");
    assert!(
        active_entry.active_users >= 1,
        "Active HS should have at least 1 active user"
    );

    // The orphan homeserver must have 0 active users
    let orphan_entry = hs_entries
        .iter()
        .find(|e| e.id == orphan_id.as_str())
        .expect("Orphan HS entry should be present");
    assert_eq!(
        orphan_entry.active_users, 0,
        "Orphan HS should have 0 active users"
    );

    // The list should be sorted by active_users descending
    for window in hs_entries.windows(2) {
        assert!(
            window[0].active_users >= window[1].active_users,
            "Homeservers should be sorted by active_users descending"
        );
    }

    // Cleanup
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
