use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::{
    bookmark_uri_builder, post_uri_builder,
    traits::{HasIdPath, HashId},
    PubkyAppBookmark, PubkyAppPost, PubkyAppUser,
};
use tracing::error;

/// The user profile is stored in the homeserver. Missing the author that creates the bookmark
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_bookmark_without_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let author_kp = Keypair::random();
    let author = PubkyAppUser {
        bio: Some("test_homeserver_bookmark_without_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:User:Sync".to_string(),
        status: None,
    };
    let author_id = test.create_user(&author_kp, &author).await?;

    let post = PubkyAppPost {
        content: "Watcher:Bookmark:User:Sync:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&author_kp, &post).await?;

    // Create a key but it would not be synchronised in nexus
    let shadow_user_kp = Keypair::random();
    let shadow_user_id = shadow_user_kp.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&shadow_user_kp).await?;

    // Create a bookmark content
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(author_id, post_id),
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    // Create the bookmark of the shadow user
    let bookmark_id = bookmark.create_id();
    let bookmark_relative_url = PubkyAppBookmark::create_path(&bookmark_id);
    let bookmark_absolute_url = bookmark_uri_builder(shadow_user_id, bookmark_id);

    // Switch OFF the event processor to simulate the pending events to index
    test = test.remove_event_processing().await;
    // Put bookmark
    test.put(&shadow_user_kp, &bookmark_relative_url, bookmark)
        .await?;

    // Create raw event line to retrieve the content from the homeserver
    let bookmark_event = format!("PUT {bookmark_absolute_url}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let moderation_ref = test.event_processor_runner.moderation.clone();
    let sync_fail = retrieve_and_handle_event_line(&bookmark_event, moderation_ref)
        .await
        .map_err(|e| error!("SYNC ERROR: {:?}", e))
        .is_err();

    assert!(
        sync_fail,
        "It seems that post node exists, which should not be possible. Event processor should be disconnected"
    );

    Ok(())
}
