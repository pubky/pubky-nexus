use crate::utils::watcher::{assert_eventually_exists, WatcherTest};
use anyhow::Result;
use nexus_watcher::events::{errors::EventProcessorError, retry::event::RetryEvent, EventType};
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppBookmark, PubkyAppUser};

/// The user profile is stored in the homeserver. Missing the post to connect the bookmark
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

    // Create the bookmark of the shadow user
    let bookmark_id = bookmark.create_id();
    let bookmark_url = format!("pubky://{user_id}/pub/pubky.app/bookmarks/{bookmark_id}");
    // PUT bookmark
    test.put(&bookmark_url, bookmark).await?;

    let put_index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&bookmark_url).unwrap()
    );

    assert_eventually_exists(&put_index_key).await;

    let timestamp = RetryEvent::check_uri(&put_index_key).await.unwrap();
    assert!(timestamp.is_some());

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
        _ => panic!("The error type has to be MissingDependency type"),
    };

    // DEL bookmark
    test.del(&bookmark_url).await?;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&bookmark_url).unwrap()
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
