use crate::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use nexus_common::types::DynError;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUser};
use tracing::error;

/// The user profile is stored in the homeserver and synched in the graph, but the posts just exist in the homeserver
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_repost_without_post_parent() -> Result<(), DynError> {
    let mut test = WatcherTest::setup().await?;

    let post_author_user_keypair = Keypair::random();
    let post_author = PubkyAppUser {
        bio: Some("test_homeserver_post_repost_without_post_parent".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostRepostFail:PostAuthor".to_string(),
        status: None,
    };
    let post_author_id = test
        .create_user(&post_author_user_keypair, &post_author)
        .await?;

    let post_repost_author_keypair = Keypair::random();
    let repost_author = PubkyAppUser {
        bio: Some("test_homeserver_post_repost_without_post_parent".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostRepostFail:RepostAuthor".to_string(),
        status: None,
    };
    let repost_author_id = test
        .create_user(&post_repost_author_keypair, &repost_author)
        .await?;

    // Switch OFF the event processor to simulate the pending events to index
    test = test.remove_event_processing().await;

    let post = PubkyAppPost {
        content: "Watcher:PostRepostFail:PostAuthor:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&post_author_id, &post).await?;

    // Create repost
    let repost = PubkyAppPost {
        content: "Watcher:PostRepostFail:RepostAuthor:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: format!("pubky://{post_author_id}/pub/pubky.app/posts/{post_id}"),
        }),
        attachments: None,
    };
    let repost_id = test.create_post(&repost_author_id, &repost).await?;

    // Create raw event line to retrieve the content from the homeserver
    let post_homeserver_uri =
        format!("PUT pubky://{repost_author_id}/pub/pubky.app/posts/{repost_id}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let sync_fail = retrieve_and_handle_event_line(&post_homeserver_uri)
        .await
        .map_err(|e| {
            error!("SYNC ERROR: {:?}", e);
        })
        .is_err();

    assert!(
        sync_fail,
        "It seems that post repost relationships exists, which should not be possible. Event processor should be disconnected"
    );

    Ok(())
}
