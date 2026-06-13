use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::{follow_uri_builder, PubkyAppUser};
use tracing::error;

/// Verifies that a follow fails with MissingDependency when either party is not yet indexed.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_follow_cannot_complete() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let follower_kp = Keypair::random();
    let follower = PubkyAppUser {
        bio: Some("test_homeserver_follow_cannot_complete".to_string()),
        image: None,
        links: None,
        name: "Watcher:CannotFollow:Follower:Sync".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_kp, &follower).await?;

    // Switch OFF event processing — followee signs up but is not indexed
    test = test.remove_event_processing().await;

    let followee_kp = Keypair::random();
    let followee = PubkyAppUser {
        bio: Some("test_homeserver_follow_cannot_complete".to_string()),
        image: None,
        links: None,
        name: "Watcher:CannotFollow:Followee:Unsync".to_string(),
        status: None,
    };
    let shadow_followee_id = test.create_user(&followee_kp, &followee).await?;

    let _follow_path = test
        .create_follow(&follower_kp, &shadow_followee_id)
        .await?;

    // Full URI required by Event::parse_event
    let follow_event = format!(
        "PUT {}",
        follow_uri_builder(follower_id.clone(), shadow_followee_id.clone())
    );

    let event_handler = test.event_processor_runner.event_handler.clone();
    let sync_fail = retrieve_and_handle_event_line(&follow_event, event_handler)
        .await
        .map_err(|e| error!("SYNC ERROR: {:?}", e))
        .is_err();

    assert!(
        sync_fail,
        "Follow indexing should fail: followee is not yet indexed"
    );

    // Opposite direction: followee follows follower (follower IS indexed, followee is NOT)
    let _opposite_follow_path = test.create_follow(&followee_kp, &follower_id).await?;

    let opposite_follow_event = format!(
        "PUT {}",
        follow_uri_builder(shadow_followee_id, follower_id)
    );

    let event_handler = test.event_processor_runner.event_handler.clone();
    let sync_fail = retrieve_and_handle_event_line(&opposite_follow_event, event_handler)
        .await
        .map_err(|e| error!("SYNC ERROR: {:?}", e))
        .is_err();

    assert!(
        sync_fail,
        "Follow indexing should fail: follower (shadow) is not yet indexed"
    );

    Ok(())
}
