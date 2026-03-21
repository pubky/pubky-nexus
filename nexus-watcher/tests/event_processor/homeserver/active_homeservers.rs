use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::exec_single_row;
use nexus_common::db::queries;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyId};

#[tokio_shared_rt::test(shared)]
async fn test_get_all_active_excludes_orphan_homeservers() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    // Create an orphan homeserver (no users hosted on it)
    let orphan_keys = Keypair::random();
    let orphan_id = PubkyId::try_from(&orphan_keys.public_key().to_z32())?;
    let orphan_hs = Homeserver::new(orphan_id.clone());
    orphan_hs.put_to_graph().await?;

    // Create a user via WatcherTest, which persists the user in the graph
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_get_all_active_excludes_orphan".to_string()),
        image: None,
        links: None,
        name: "Watcher:ActiveHS:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Link the user to the test homeserver via HOSTED_BY
    let default_id = test.homeserver_id.clone();
    let link_query = queries::put::set_user_homeserver(&user_id, &default_id);
    exec_single_row(link_query).await?;

    // Query active homeservers
    let hs_ids = Homeserver::get_all_active_from_graph().await?;

    // The test homeserver must be present (it has a user linked via HOSTED_BY)
    assert!(hs_ids.contains(&default_id), "Active HS should be included");
    // The orphan homeserver must NOT be present
    assert!(!hs_ids.contains(&orphan_id), "Orphan HS should be excluded");

    // Cleanup
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
