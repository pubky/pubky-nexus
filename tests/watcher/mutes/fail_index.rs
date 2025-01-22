use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::PubkyAppUser;
use pubky_common::crypto::Keypair;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_mute_cannot_complete() -> Result<()> {
    let mut test = WatcherTest::setup(false).await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_mute_cannot_complete".to_string()),
        image: None,
        links: None,
        name: "Watcher:Mute:User:Sync".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Create a key but it would not be synchronised in nexus
    let shadow_keypair = Keypair::random();
    let shadow_user_id = shadow_keypair.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&shadow_keypair).await?;

    // Mute the user
    let muted_uri = test.create_mute(&user_id, &shadow_user_id).await?;
    // Unmute the user
    test.del(&muted_uri).await?;

    // Create a mute in opposite direction
    let opossite_muted_uri = test.create_mute(&shadow_user_id, &user_id).await?;
    // Unmute the user
    test.del(&opossite_muted_uri).await?;

    Ok(())
}
