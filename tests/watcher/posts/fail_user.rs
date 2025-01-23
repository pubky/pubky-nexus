use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind};
use pubky_common::crypto::Keypair;

/// The user profile is stored in the homeserver. Missing the author to connect the post
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_without_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user_id = keypair.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&keypair).await?;

    let post = PubkyAppPost {
        content: "Watcher:PostEvent:PostWithoutUser".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&user_id, &post).await?;

    // Delete the post using the event handler
    test.cleanup_post(&user_id, &post_id).await?;

    Ok(())
}
