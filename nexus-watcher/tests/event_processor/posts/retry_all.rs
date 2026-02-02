use crate::event_processor::utils::watcher::{assert_eventually_exists, WatcherTest};
use anyhow::Result;
use nexus_common::models::event::{EventProcessorError, EventType};
use nexus_watcher::events::retry::event::RetryEvent;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser,
};
/// The user profile is stored in the homeserver. Missing the post to connect the new one
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_with_reply_repost_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_reply".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:PostRepost:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&user_kp, &user).await?;

    // Use a placeholder parent post ID to intentionally avoid resolving it in the graph database
    let reply_fake_post_id = "0032QB10HCRHG";
    let repost_fake_post_id = "0032QB10HP6JJ";
    // Create parent post uri
    let reply_absolute_uri = post_uri_builder(user_id.clone(), reply_fake_post_id.into());
    let repost_absolute_uri = post_uri_builder(user_id.clone(), repost_fake_post_id.into());

    let repost_reply = PubkyAppPost {
        content: "Watcher:IndexFail:PostRepost:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(reply_absolute_uri.clone()),
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: repost_absolute_uri.clone(),
        }),
        attachments: None,
    };

    let (repost_reply_id, repost_reply_path) = test.create_post(&user_kp, &repost_reply).await?;

    let repost_reply_absolute_url = post_uri_builder(user_id, repost_reply_id);

    let index_key = format!(
        "{}:{}",
        EventType::Put,
        RetryEvent::generate_index_key(&repost_reply_absolute_url).unwrap()
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
            assert_eq!(dependency.len(), 2);
            assert_eq!(
                dependency[0],
                RetryEvent::generate_index_key(&reply_absolute_uri).unwrap()
            );
            assert_eq!(
                dependency[1],
                RetryEvent::generate_index_key(&repost_absolute_uri).unwrap()
            );
        }
        _ => panic!("The error type has to be MissingDependency type"),
    };

    test.del(&user_kp, &repost_reply_path).await?;

    let del_index_key = format!(
        "{}:{}",
        EventType::Del,
        RetryEvent::generate_index_key(&repost_reply_absolute_url).unwrap()
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
