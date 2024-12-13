use super::utils::find_mute_relationship;
use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky_app_specs::PubkyAppUser;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::user::{Muted, Relationship};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_mute() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (muter)
    let muter_keypair = Keypair::random();
    let muter_user = PubkyAppUser {
        bio: Some("test_homeserver_mute_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:Mute:Muter".to_string(),
        status: None,
    };
    let muter_id = test.create_user(&muter_keypair, &muter_user).await?;

    // Create second user (mutee)
    let mutee_keypair = Keypair::random();
    let mutee_user = PubkyAppUser {
        bio: Some("test_homeserver_mute_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:Mute:Mutee".to_string(),
        status: None,
    };
    let mutee_id = test.create_user(&mutee_keypair, &mutee_user).await?;

    // Mute the user
    test.create_mute(&muter_id, &mutee_id).await?;

    // Assert if the mute relationship was created
    let exist = find_mute_relationship(&muter_id, &mutee_id).await?;
    assert!(exist, "The mute relationship was not created in the Graph");

    // CACHE_OP: Assert if cache has been updated
    let muted = Muted::check(&muter_id, &mutee_id).await.unwrap();
    assert!(
        muted,
        "Mutee should be present in the muter user's mute list"
    );

    // UserRelationships
    let relationship = Relationship::get_by_id(&mutee_id, Some(&muter_id))
        .await
        .unwrap()
        .unwrap();

    assert!(
        relationship.muted,
        "The user relationship between muter and mutee should be muted=true"
    );

    // Cleanup
    test.cleanup_user(&muter_id).await?;
    test.cleanup_user(&mutee_id).await?;

    Ok(())
}
