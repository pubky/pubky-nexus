use crate::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use nexus_common::types::DynError;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};
use tracing::error;

/// The user profile is stored in the homeserver and synched in the graph, but the posts just exist in the homeserver
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_reply_without_post_parent() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    let author_user_keypair = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_homeserver_post_reply_without_post_parent".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostReplyFail:Author".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_user_keypair, &author).await?;

    // Switch OFF the event processor to simulate the pending events to index
    test = test.remove_event_processing().await;

    let post = PubkyAppPost {
        content: "Watcher:PostReplyFail:Author:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&author_id, &post).await?;

    // Create reply
    let parent_uri = format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}");

    let reply = PubkyAppPost {
        content: "Watcher:PostReplyFail:Author:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
        attachments: None,
    };

    let reply_id = test.create_post(&author_id, &reply).await?;

    // Create raw event line to retrieve the content from the homeserver
    let post_event = format!("PUT pubky://{author_id}/pub/pubky.app/posts/{reply_id}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let sync_fail = retrieve_and_handle_event_line(&post_event)
        .await
        .map_err(|e| {
            error!("SYNC ERROR: {:?}", e);
        })
        .is_err();

    assert!(
        sync_fail,
        "It seems that post reply relationships exists, which should not be possible. Event processor should be disconnected"
    );

    Ok(())
}
