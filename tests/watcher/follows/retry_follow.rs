use std::time::Duration;

use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::PubkyAppUser;
use pubky_common::crypto::Keypair;
use pubky_nexus::events::{error::EventProcessorError, retry::event::RetryEvent, EventType};

/// The user profile is stored in the homeserver. Missing the followee to connect with follower
// These types of tests (e.g., retry_xxxx) can be used to verify whether the `RetryManager`
// cache correctly adds the events as expected.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_follow_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let followee_keypair = Keypair::random();
    let followee_id = followee_keypair.public_key().to_z32();
    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&followee_keypair).await?;

    let follower_keypair = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("test_homeserver_follow_cannot_index".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:Follower".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_keypair, &follower_user).await?;

    // Mute the user
    test.create_follow(&follower_id, &followee_id).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let follow_url = format!("pubky://{follower_id}/pub/pubky.app/follows/{followee_id}");

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&follow_url).unwrap()
    );

    // Assert if the event is in the timeline
    let timestamp = RetryEvent::check_uri(&index_key).await.unwrap();
    assert!(timestamp.is_some());

    // Assert if the event is in the state hash map
    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    let dependency_uri = format!("{followee_id}:user:profile.json");

    match event_state.error_type {
        EventProcessorError::MissingDependency { dependency } => {
            assert_eq!(dependency.len(), 1);
            assert_eq!(dependency[0], dependency_uri)
        }
        _ => assert!(false, "The error type has to be MissingDependency type"),
    };

    test.del(&follow_url).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&follow_url).unwrap()
    );

    // Assert that the event does not exist in the sorted set. In that case PUT event
    let timestamp = RetryEvent::check_uri(&del_index_key).await.unwrap();
    assert!(timestamp.is_some());

    // Assert if the event is in the state. JSON
    let event_retry = RetryEvent::get_from_index(&del_index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    match event_state.error_type {
        EventProcessorError::SkipIndexing => (),
        _ => assert!(false, "The error type has to be SkipIndexing type"),
    };

    Ok(())
}
