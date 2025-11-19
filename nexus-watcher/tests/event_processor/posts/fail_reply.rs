use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use nexus_common::types::DynError;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppUser};
use tracing::error;

/// The user profile is stored in the homeserver and synched in the graph, but the posts just exist in the homeserver
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_reply_without_post_parent() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    let author_user_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_homeserver_post_reply_without_post_parent".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostReplyFail:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_user_kp, &author).await?;

    // Switch OFF the event processor to simulate the pending events to index
    test = test.remove_event_processing().await;

    let post = PubkyAppPost {
        content: "Watcher:PostReplyFail:Author:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let (post_id, _post_path) = test.create_post(&author_user_kp, &post).await?;

    // Create reply
    let parent_absolute_uri = post_uri_builder(author_id.clone(), post_id);

    let reply = PubkyAppPost {
        content: "Watcher:PostReplyFail:Author:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_absolute_uri.clone()),
        embed: None,
        attachments: None,
    };

    let (reply_id, _reply_path) = test.create_post(&author_user_kp, &reply).await?;

    // Create raw event line to retrieve the content from the homeserver
    let reply_absolute_uri = post_uri_builder(author_id.clone(), reply_id);
    let post_event = format!("PUT {reply_absolute_uri}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let moderation_ref = test.event_processor_runner.moderation.clone();
    let sync_fail = retrieve_and_handle_event_line(&post_event, moderation_ref)
        .await
        .map_err(|e| error!("SYNC ERROR: {:?}", e))
        .is_err();

    assert!(
        sync_fail,
        "It seems that post reply relationships exists, which should not be possible. Event processor should be disconnected"
    );

    Ok(())
}
