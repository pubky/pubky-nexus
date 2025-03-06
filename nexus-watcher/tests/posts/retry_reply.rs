use crate::utils::watcher::{assert_eventually_exists, WatcherTest};
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};
use nexus_watcher::events::{retry::event::RetryEvent, EventType};
use nexus_common::types::errors::EventProcessorError;

/// The user profile is stored in the homeserver. Missing the post to connect the new one
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_reply_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_reply".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:PostReply:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    // Use a placeholder parent post ID to intentionally avoid resolving it in the graph database
    let parent_fake_post_id = "0032QB10HCRHG";
    // Create parent post uri
    let dependency_uri = format!("pubky://{user_id}/pub/pubky.app/posts/{parent_fake_post_id}");

    let reply_post = PubkyAppPost {
        content: "Watcher:IndexFail:PostReply:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(dependency_uri.clone()),
        embed: None,
        attachments: None,
    };

    let reply_id = test.create_post(&user_id, &reply_post).await?;

    let reply_url = format!("pubky://{user_id}/pub/pubky.app/posts/{reply_id}");

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&reply_url).unwrap()
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

    test.del(&reply_url).await?;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&reply_url).unwrap()
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
