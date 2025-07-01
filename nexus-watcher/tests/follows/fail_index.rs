use crate::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;
use tracing::error;

/// The follower user is stored in the homeserver but it is not in sync with the graph
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_follow_cannot_complete() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let follower_keypair = Keypair::random();
    let follower = PubkyAppUser {
        bio: Some("test_homeserver_follow_cannot_complete".to_string()),
        image: None,
        links: None,
        name: "Watcher:CannotFollow:Follower:Sync".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_keypair, &follower).await?;

    // Switch OFF the event processor to simulate the pending events to index
    test = test.remove_event_processing().await;

    // Create a key but it would not be synchronised in the graph
    let followeee_keypair = Keypair::random();
    let followee = PubkyAppUser {
        bio: Some("test_homeserver_follow_cannot_complete".to_string()),
        image: None,
        links: None,
        name: "Watcher:CannotFollow:Followee:Unsync".to_string(),
        status: None,
    };
    let shadow_followee_id = test.create_user(&followeee_keypair, &followee).await?;

    let follow_url = test
        .create_follow(&follower_id, &shadow_followee_id)
        .await?;

    // Create raw event line to retrieve the content from the homeserver
    let follow_event = format!("PUT {follow_url}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let sync_fail = retrieve_and_handle_event_line(&follow_event)
        .await
        .map_err(|e| {
            error!("SYNC ERROR: {:?}", e);
        })
        .is_err();

    assert!(
        sync_fail,
        "It seems that relationship exists, which should not be possible. Event processor should be disconnected"
    );

    // Create a follow in opposite direction
    let opposite_follow = test
        .create_follow(&shadow_followee_id, &follower_id)
        .await?;

    // Create raw event line to retrieve the content from the homeserver
    let opposite_follow_event = format!("PUT {opposite_follow}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let sync_fail = retrieve_and_handle_event_line(&opposite_follow_event)
        .await
        .map_err(|e| {
            error!("SYNC ERROR: {:?}", e);
        })
        .is_err();

    assert!(
        sync_fail,
        "It seems that relationship exists, which should not be possible. Event processor should be disconnected"
    );

    Ok(())
}
