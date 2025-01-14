use std::time::Duration;

use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky_app_specs::{traits::HashId, PubkyAppTag, PubkyAppUser};
use pubky_common::crypto::Keypair;
use pubky_nexus::events::{error::EventProcessorError, retry::event::RetryEvent, EventType};

// These types of tests (e.g., retry_xxxx) can be used to verify whether the `RetryManager`
// cache correctly adds the events as expected.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_tag_event_to_queue() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let tagger_keypair = Keypair::random();
    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_user_tag_event_to_queue".to_string()),
        image: None,
        links: None,
        name: "Watcher:Retry:Post:CannotTag:Tagger:Sync".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_keypair, &tagger_user).await?;

    // Create a key but it would not be synchronised in nexus
    let author_keypair = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_homeserver_user_tag_event_to_queue".to_string()),
        image: None,
        links: None,
        name: "Watcher:Retry:Post:CannotTag:Author:Sync".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_keypair, &author).await?;

    // => Create user tag
    let label = "peak_limit";

    let dependency_uri = format!(
        "pubky://{author_id}/pub/pubky.app/posts/{}",
        "0032Q4SFBFD4G"
    );

    // Create a tag in a fake post
    let tag = PubkyAppTag {
        uri: dependency_uri.clone(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!(
        "pubky://{tagger_user_id}/pub/pubky.app/tags/{}",
        tag.create_id()
    );

    // PUT user tag
    // That operation is going to write the event in the pending events queue, so block a bit the thread
    // to let write the indexes
    test.put(tag_url.as_str(), tag_blob.clone()).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let index_key = format!(
        "{} {}",
        EventType::Put,
        RetryEvent::generate_index_key(&tag_url).unwrap()
    );

    // Assert if the event is in the timeline
    let timestamp = RetryEvent::check_uri(&index_key).await.unwrap();
    assert!(timestamp.is_some());

    // Assert if the event is in the state hash map
    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();

    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    match event_state.error_type {
        EventProcessorError::MissingDependency { dependency } => {
            assert_eq!(dependency.len(), 1);
            assert_eq!(dependency[0], dependency_uri);
        }
        _ => assert!(false, "The error type has to be MissingDependency type"),
    };

    Ok(())
}
