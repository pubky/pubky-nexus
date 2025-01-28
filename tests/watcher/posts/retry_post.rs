use std::time::Duration;

use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind};
use pubky_common::crypto::Keypair;
use pubky_nexus::events::{error::EventProcessorError, retry::event::RetryEvent, EventType};

/// The user profile is stored in the homeserver. Missing the author to connect the post
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user_id = keypair.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&keypair).await?;

    let post = PubkyAppPost {
        content: "Watcher:IndexFail:PostEvent:PostWithoutUser".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&user_id, &post).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let post_url = format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}");

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&post_url).unwrap()
    );

    let timestamp = RetryEvent::check_uri(&index_key).await.unwrap();
    assert!(timestamp.is_some());

    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    let dependency_uri = format!("pubky://{user_id}/pub/pubky.app/profile.json");

    match event_state.error_type {
        EventProcessorError::MissingDependency { dependency } => {
            assert_eq!(dependency.len(), 1);
            assert_eq!(
                dependency[0],
                RetryEvent::generate_index_key(&dependency_uri).unwrap()
            );
        }
        _ => panic!("The error type has to be MissingDependency type"),
    };

    test.del(&post_url).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&post_url).unwrap()
    );

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
