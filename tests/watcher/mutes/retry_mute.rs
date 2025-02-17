use crate::watcher::utils::watcher::{assert_eventually_exists, WatcherTest};
use anyhow::Result;
use pkarr::Keypair;
use pubky_app_specs::{user_uri_builder, PubkyAppUser};
use pubky_nexus::events::{error::EventProcessorError, retry::event::RetryEvent, EventType};

/// The user profile is stored in the homeserver. Missing the mutee to connect with muter
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_mute_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let mutee_keypair = Keypair::random();
    let mutee_id = mutee_keypair.public_key().to_z32();
    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&mutee_keypair).await?;

    let muter_keypair = Keypair::random();
    let muter_user = PubkyAppUser {
        bio: Some("test_homeserver_mute_cannot_index".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:Muter".to_string(),
        status: None,
    };
    let muter_id = test.create_user(&muter_keypair, &muter_user).await?;

    // Mute the user
    test.create_mute(&muter_id, &mutee_id).await?;

    let mute_url = format!("pubky://{muter_id}/pub/pubky.app/mutes/{mutee_id}");

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&mute_url).unwrap()
    );

    assert_eventually_exists(&index_key).await;

    let timestamp = RetryEvent::check_uri(&index_key).await.unwrap();
    assert!(timestamp.is_some());

    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    let dependency_key = RetryEvent::generate_index_key(&user_uri_builder(mutee_id.to_string()));

    match event_state.error_type {
        EventProcessorError::MissingDependency { dependency } => {
            assert_eq!(dependency.len(), 1);
            assert_eq!(dependency[0], dependency_key.unwrap())
        }
        _ => panic!("The error type has to be MissingDependency type"),
    };

    test.del(&mute_url).await?;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&mute_url).unwrap()
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
