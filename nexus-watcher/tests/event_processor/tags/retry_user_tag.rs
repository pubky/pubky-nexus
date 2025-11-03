use crate::event_processor::utils::watcher::{assert_eventually_exists, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::event::EventType;
use nexus_watcher::events::errors::EventProcessorError;
use nexus_watcher::events::retry::event::RetryEvent;

use pubky::Keypair;
use pubky_app_specs::tag_uri_builder;
use pubky_app_specs::{traits::HashId, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_user_tag_event_to_queue() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let tagger_keypair = Keypair::random();
    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_user_tag_event_to_queue".to_string()),
        image: None,
        links: None,
        name: "Watcher:Retry:User:CannotTag:Tagger:Sync".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_keypair, &tagger_user).await?;

    // Create a user key but it would not be synchronised in nexus
    let shadow_keypair = Keypair::random();
    test.register_user(&shadow_keypair).await?;
    let shadow_user_id = shadow_keypair.public_key().to_z32();

    // => Create user tag
    let label = "friendly";

    let dependency_uri = format!("pubky://{shadow_user_id}/pub/pubky.app/profile.json");

    let tag = PubkyAppTag {
        uri: dependency_uri.clone(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_url = tag_uri_builder(tagger_user_id, tag.create_id());

    // PUT user tag
    // That operation is going to write the event in the pending events queue, so block a bit the thread
    // to let write the indexes
    test.put(tag_url.as_str(), tag).await?;

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&tag_url).unwrap()
    );

    assert_eventually_exists(&index_key).await;

    let timestamp = RetryEvent::check_uri(&index_key).await.unwrap();
    assert!(timestamp.is_some());

    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

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

    test.del(&tag_url).await?;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&tag_url).unwrap()
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
