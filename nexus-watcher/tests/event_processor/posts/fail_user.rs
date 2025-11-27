use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind};

/// The user profile is stored in the homeserver. Missing the author to connect the post
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_without_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let _user_id = user_kp.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&user_kp).await?;

    let post = PubkyAppPost {
        content: "Watcher:PostEvent:PostWithoutUser".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let (_post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Delete the post using the event handler
    test.cleanup_post(&user_kp, &post_path).await?;

    Ok(())
}
