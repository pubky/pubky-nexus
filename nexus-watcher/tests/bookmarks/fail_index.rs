use crate::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser};
use tracing::error;

/// The user profile is stored in the homeserver. Missing the author that creates the bookmark
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_bookmark_without_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let author_keypair = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_homeserver_bookmark_without_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:User:Sync".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_keypair, &author).await?;

    let post = PubkyAppPost {
        content: "Watcher:Bookmark:User:Sync:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&author_id, &post).await?;

    // Create a key but it would not be synchronised in nexus
    let keypair = Keypair::random();
    let shadow_user_id = keypair.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&keypair).await?;

    // Create a bookmark content
    let bookmark = PubkyAppBookmark {
        uri: format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}"),
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    // Create the bookmark of the shadow user
    let bookmark_id = bookmark.create_id();
    let bookmark_url = format!("pubky://{shadow_user_id}/pub/pubky.app/bookmarks/{bookmark_id}");

    // Switch OFF the event processor to simulate the pending events to index
    test = test.remove_event_processing().await;
    // Put bookmark
    test.put(&bookmark_url, bookmark).await?;

    // Create raw event line to retrieve the content from the homeserver
    let bookmark_event = format!("PUT {bookmark_url}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let sync_fail = retrieve_and_handle_event_line(&bookmark_event)
        .await
        .map_err(|e| {
            error!("SYNC ERROR: {:?}", e);
        })
        .is_err();

    assert!(
        sync_fail,
        "It seems that post node exists, which should not be possible. Event processor should be disconnected"
    );

    Ok(())
}
