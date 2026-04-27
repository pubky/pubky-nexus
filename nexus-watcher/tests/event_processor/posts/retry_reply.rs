use crate::event_processor::utils::watcher::{assert_eventually_exists, WatcherTest};
use anyhow::Result;
use nexus_watcher::events::retry::RetryEvent;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppUser};

/// The user profile is stored in the homeserver. Missing the post to connect the new one
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_reply_cannot_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_reply".to_string()),
        image: None,
        links: None,
        name: "Watcher:IndexFail:PostReply:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&user_kp, &user).await?;

    // Use a placeholder parent post ID to intentionally avoid resolving it in the graph database
    let parent_fake_post_id = "0032QB10HCRHG";
    // Create parent post uri
    let dependency_absolute_uri = post_uri_builder(user_id.clone(), parent_fake_post_id.into());

    let reply_post = PubkyAppPost {
        content: "Watcher:IndexFail:PostReply:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(dependency_absolute_uri.clone()),
        embed: None,
        attachments: None,
    };

    let (reply_id, _reply_path) = test.create_post(&user_kp, &reply_post).await?;

    let reply_absolute_url = post_uri_builder(user_id, reply_id);

    let index_key = reply_absolute_url.clone();

    assert_eventually_exists(&index_key).await;

    assert!(RetryEvent::check_uri(&index_key).await.unwrap());

    let event_retry = RetryEvent::get_from_index(&index_key).await.unwrap();
    assert!(event_retry.is_some());

    let event_state = event_retry.unwrap();
    assert_eq!(event_state.retry_count, 0);

    Ok(())
}
