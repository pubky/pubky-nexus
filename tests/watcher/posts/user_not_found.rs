use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind};
use pubky_common::crypto::Keypair;
use pubky_nexus::PubkyConnector;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_without_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user_id = keypair.public_key().to_z32();

    // Create a keypair but just signup in the homeserver, do not create the profile.json
    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    let pubky_client = PubkyConnector::get_pubky_client()?;
    // Register the key in the homeserver
    pubky_client
        .signup(&keypair, &test.homeserver.public_key())
        .await?;

    let post = PubkyAppPost {
        content: "Watcher:PostEvent:PostWithoutUser".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&user_id, &post).await?;

    Ok(())
}
