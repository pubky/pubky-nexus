use crate::event_processor::utils::watcher::{assert_eventually_exists, WatcherTest};
use anyhow::Result;
use nexus_common::models::event::{EventProcessorError, EventType};
use nexus_watcher::events::retry::event::RetryEvent;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppPostKind};

/// The user profile is stored in the homeserver. Missing the author to connect the post
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user_id = user_kp.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&user_kp).await?;

    let post = PubkyAppPost {
        content: "Watcher:IndexFail:PostEvent:PostWithoutUser".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    let post_absolute_url = post_uri_builder(user_id.clone(), post_id);

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&post_absolute_url).unwrap()
    );

    assert_eventually_exists(&index_key).await;

    let timestamp = RetryEvent::check_uri(&index_key).await.unwrap();
    assert!(timestamp.is_some());

    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();
    assert_eq!(event_state.retry_count, 0);

    let dependency_absolute_uri = format!("pubky://{user_id}/pub/pubky.app/profile.json");

    match event_state.error_type {
        EventProcessorError::MissingDependency { dependency } => {
            assert_eq!(dependency.len(), 1);
            assert_eq!(
                dependency[0],
                RetryEvent::generate_index_key(&dependency_absolute_uri).unwrap()
            );
        }
        _ => panic!("The error type has to be MissingDependency type"),
    };

    test.del(&user_kp, &post_path).await?;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&post_absolute_url).unwrap()
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
