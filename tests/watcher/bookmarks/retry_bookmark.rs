use std::time::Duration;

use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::{traits::HashId, PubkyAppBookmark, PubkyAppUser};
use pubky_common::crypto::Keypair;
use pubky_nexus::events::{error::EventProcessorError, retry::event::RetryEvent, EventType};

/// The user profile is stored in the homeserver. Missing the post to connect the bookmark
// These types of tests (e.g., retry_xxxx) can be used to verify whether the `RetryManager`
// cache correctly adds the events as expected.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_bookmark_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_bookmark_cannot_index".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:Bookmark:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Use a placeholder parent post ID to intentionally avoid resolving it in the graph database
    let fake_post_id = "0032QB10HCRHG";
    let fake_user_id = "ba3e8qeby33uq9cughpxdf7bew9etn1eq8bc3yhwg7p1f54yaozy";
    // Create parent post uri
    let post_uri = format!("pubky://{fake_user_id}/pub/pubky.app/posts/{fake_post_id}");

    // Create a bookmark content
    let bookmark = PubkyAppBookmark {
        uri: post_uri,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_blob = serde_json::to_vec(&bookmark)?;
    // Create the bookmark of the shadow user
    let bookmark_id = bookmark.create_id();
    let bookmark_url = format!(
        "pubky://{}/pub/pubky.app/bookmarks/{}",
        user_id, bookmark_id
    );
    // PUT bookmark
    test.put(&bookmark_url, bookmark_blob).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let put_index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&bookmark_url).unwrap()
    );

    // Assert if the event is in the timeline
    let timestamp = RetryEvent::check_uri(&put_index_key).await.unwrap();
    assert!(timestamp.is_some());

    // Assert if the event is in the state. JSON
    let event_retry = RetryEvent::get_from_index(&put_index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    let dependency_uri = format!("{fake_user_id}:posts:{fake_post_id}");
    match event_state.error_type {
        EventProcessorError::MissingDependency { dependency } => {
            assert_eq!(dependency.len(), 1);
            assert_eq!(dependency[0], dependency_uri);
        }
        _ => assert!(false, "The error type has to be MissingDependency type"),
    };

    // DEL bookmark
    test.del(&bookmark_url).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&bookmark_url).unwrap()
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
