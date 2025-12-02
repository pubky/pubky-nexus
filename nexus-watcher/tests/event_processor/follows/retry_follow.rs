use crate::event_processor::utils::watcher::{assert_eventually_exists, WatcherTest};
use anyhow::Result;
use nexus_common::models::event::{EventProcessorError, EventType};
use nexus_watcher::events::retry::event::RetryEvent;
use pubky::Keypair;
use pubky_app_specs::{follow_uri_builder, PubkyAppUser, PubkyId};

/// The user profile is stored in the homeserver. Missing the followee to connect with follower
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_follow_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let followee_keypair = Keypair::random();
    let followee_id = followee_keypair.public_key().to_z32();
    let followee_pubky_id = PubkyId::try_from(&followee_id).unwrap();
    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&followee_keypair).await?;

    let follower_kp = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("test_homeserver_follow_cannot_index".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:Follower".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&follower_kp, &follower_user).await?;

    let follow_path = test.create_follow(&follower_kp, &followee_id).await?;
    let follow_absolute_url = follow_uri_builder(follower_id, followee_id.clone());

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&follow_absolute_url).unwrap()
    );
    assert_eventually_exists(&index_key).await;

    let timestamp = RetryEvent::check_uri(&index_key).await.unwrap();
    assert!(timestamp.is_some());

    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    let dependency_key = RetryEvent::generate_index_key_from_uri(&followee_pubky_id.to_uri());

    match event_state.error_type {
        EventProcessorError::MissingDependency { dependency } => {
            assert_eq!(dependency.len(), 1);
            assert_eq!(dependency[0], dependency_key)
        }
        _ => panic!("The error type has to be MissingDependency type"),
    };

    test.del(&follower_kp, &follow_path).await?;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&follow_absolute_url).unwrap()
    );

    assert_eventually_exists(&del_index_key).await;

    let timestamp = RetryEvent::check_uri(&del_index_key).await.unwrap();
    assert!(timestamp.is_some());

    let event_retry = RetryEvent::get_from_index(&del_index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    match event_state.error_type {
        EventProcessorError::SkipIndexing => (),
        _ => panic!("The error type has to be SkipIndexing type"),
    };

    Ok(())
}
