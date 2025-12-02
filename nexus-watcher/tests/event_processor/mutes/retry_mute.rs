use crate::event_processor::utils::watcher::{assert_eventually_exists, WatcherTest};
use anyhow::Result;
use nexus_common::models::event::{EventProcessorError, EventType};
use nexus_watcher::events::retry::event::RetryEvent;
use pubky::Keypair;
use pubky_app_specs::{mute_uri_builder, PubkyAppUser, PubkyId};
/// The user profile is stored in the homeserver. Missing the mutee to connect with muter
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_mute_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let mutee_kp = Keypair::random();
    let mutee_id = mutee_kp.public_key().to_z32();
    let mutee_pubky_id = PubkyId::try_from(&mutee_id).unwrap();
    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&mutee_kp).await?;

    let muter_kp = Keypair::random();
    let muter_user = PubkyAppUser {
        bio: Some("test_homeserver_mute_cannot_index".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:Muter".to_string(),
        status: None,
    };
    let muter_id = test.create_user(&muter_kp, &muter_user).await?;

    // Mute the user
    let mute_path = test.create_mute(&muter_kp, &mutee_id).await?;

    let mute_absolute_url = mute_uri_builder(muter_id, mutee_id.clone());

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&mute_absolute_url).unwrap()
    );

    assert_eventually_exists(&index_key).await;

    let timestamp = RetryEvent::check_uri(&index_key).await.unwrap();
    assert!(timestamp.is_some());

    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    let dependency_key = RetryEvent::generate_index_key_from_uri(&mutee_pubky_id.to_uri());

    match event_state.error_type {
        EventProcessorError::MissingDependency { dependency } => {
            assert_eq!(dependency.len(), 1);
            assert_eq!(dependency[0], dependency_key)
        }
        _ => panic!("The error type has to be MissingDependency type"),
    };

    test.del(&muter_kp, &mute_path).await?;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&mute_absolute_url).unwrap()
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
