use std::time::Duration;

use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser};
use pubky_common::crypto::Keypair;
use pubky_nexus::events::{error::EventProcessorError, retry::event::RetryEvent, EventType};

/// The user profile is stored in the homeserver. Missing the post to connect the new one
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_with_reply_repost_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_reply".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:PostRepost:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    // Use a placeholder parent post ID to intentionally avoid resolving it in the graph database
    let reply_fake_post_id = "0032QB10HCRHG";
    let repost_fake_post_id = "0032QB10HP6JJ";
    // Create parent post uri
    let reply_uri = format!("pubky://{user_id}/pub/pubky.app/posts/{reply_fake_post_id}");
    let repost_uri = format!("pubky://{user_id}/pub/pubky.app/posts/{repost_fake_post_id}");

    let repost_reply_post = PubkyAppPost {
        content: "Watcher:IndexFail:PostRepost:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(reply_uri.clone()),
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: repost_uri.clone(),
        }),
        attachments: None,
    };

    let repost_reply_post_id = test.create_post(&user_id, &repost_reply_post).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let repost_reply_url = format!("pubky://{user_id}/pub/pubky.app/posts/{repost_reply_post_id}");

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&repost_reply_url).unwrap()
    );

    let timestamp = RetryEvent::check_uri(&index_key).await.unwrap();
    assert!(timestamp.is_some());

    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();

    assert_eq!(event_state.retry_count, 0);

    match event_state.error_type {
        EventProcessorError::MissingDependency { dependency } => {
            assert_eq!(dependency.len(), 2);
            assert_eq!(
                dependency[0],
                RetryEvent::generate_index_key(&reply_uri).unwrap()
            );
            assert_eq!(
                dependency[1],
                RetryEvent::generate_index_key(&repost_uri).unwrap()
            );
        }
        _ => assert!(false, "The error type has to be MissingDependency type"),
    };

    test.del(&repost_reply_url).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&repost_reply_url).unwrap()
    );

    let timestamp = RetryEvent::check_uri(&del_index_key).await.unwrap();
    assert!(timestamp.is_some());

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
