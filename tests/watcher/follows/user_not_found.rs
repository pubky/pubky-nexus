use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::PubkyAppUser;
use pubky_common::crypto::Keypair;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_follow_cannot_complete() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_follow_cannot_complete".to_string()),
        image: None,
        links: None,
        name: "Watcher:Follow:User:Sync".to_string(),
        status: None,
    };
    let follower_id = test.create_user(&keypair, &user).await?;

    // Create a key but it would not be synchronised in nexus
    let keypair = Keypair::random();
    let shadow_followee_id = keypair.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&keypair).await?;
    
    // NOTE: All that events are going to throw an error because the shadow followee does not exist
    // Follow the followee
    let follow_url = test.create_follow(&follower_id, &shadow_followee_id).await?;
    test.del(&follow_url).await?;

    // Create a follow in opposite direction
    let opposite_follow = test.create_follow(&shadow_followee_id, &follower_id).await?;
    test.del(&opposite_follow).await?;

    Ok(())
}
