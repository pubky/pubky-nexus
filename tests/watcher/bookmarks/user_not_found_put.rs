use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::{traits::HashId, PubkyAppBookmark, PubkyAppPost, PubkyAppUser};
use pubky_common::crypto::Keypair;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_bookmark_without_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_bookmark_without_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:Bookmark:User:Sync".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:Bookmark:User:Sync:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Create a key but it would not be synchronised in nexus
    let keypair = Keypair::random();
    let shadow_user_id = keypair.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&keypair).await?;

    // Create a bookmark content
    let bookmark = PubkyAppBookmark {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_blob = serde_json::to_vec(&bookmark)?;
    // Create the bookmark of the shadow user
    let bookmark_id = bookmark.create_id();
    let bookmark_url = format!(
        "pubky://{}/pub/pubky.app/bookmarks/{}",
        shadow_user_id, bookmark_id
    );

    // Put bookmark
    test.put(&bookmark_url, bookmark_blob).await?;

    Ok(())
}
