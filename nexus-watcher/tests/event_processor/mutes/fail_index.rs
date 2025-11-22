use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_mute_cannot_complete() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_mute_cannot_complete".to_string()),
        image: None,
        links: None,
        name: "Watcher:Mute:User:Sync".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Create a key but it would not be synchronised in nexus
    let shadow_user_kp = Keypair::random();
    let shadow_user_id = shadow_user_kp.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&shadow_user_kp).await?;

    // Mute the user
    let muted_uri = test.create_mute(&user_kp, &shadow_user_id).await?;
    // Unmute the user
    test.del(&user_kp, &muted_uri).await?;

    // Create a mute in opposite direction
    let opossite_muted_uri = test.create_mute(&shadow_user_kp, &user_id).await?;
    // Unmute the user
    test.del(&shadow_user_kp, &opossite_muted_uri).await?;

    Ok(())
}
