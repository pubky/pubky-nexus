use crate::watcher::utils::watcher::{create_event_from_uri, WatcherTest};
use anyhow::Result;
use log::error;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};
use pubky_common::crypto::Keypair;
use pubky_nexus::types::DynError;

/// The user profile is stored in the homeserver, but for some reason, the indexer failed to ingest it
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_reply_without_post_parent() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    let author_user_keypair = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_homeserver_post_reply_without_post_parent".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostReplyFail:Miss:Alice".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_user_keypair, &author).await?;

    // Switch OFF the event processor
    test = test.remove_event_processing().await;

    let post = PubkyAppPost {
        content: "Watcher:PostReplyFail:Deleted:Alice:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&author_id, &post).await?;

    // Create reply
    let parent_uri = format!(
        "pubky://{}/pub/pubky.app/posts/{}",
        author_id, post_id
    );

    let reply = PubkyAppPost {
        content: "Watcher:PostReplyFail:Deleted:Alice:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
        attachments: None,
    };

    let reply_id = test.create_post(&author_id, &reply).await?;

    // Create raw event line to retrieve the content from the homeserver. Event processor is deactivated
    // Like this, we can trigger the error in that test
    let post_homeserver_uri = format!(
        "PUT pubky://{}/pub/pubky.app/posts/{}",
        author_id, reply_id
    );
    
    let sync_fail = create_event_from_uri(&post_homeserver_uri)
        .await
        .map_err(|e| {
            error!("SYNC ERROR: {:?}", e);
        })
        .is_err();

    assert!(sync_fail, "Cannot exist the parent post because it is not in sync the graph with events");

    Ok(())
}
