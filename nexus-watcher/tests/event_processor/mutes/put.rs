use super::utils::find_mute_relationship;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::user::{Muted, Relationship};
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_mute() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create first user (muter)
    let muter_kp = Keypair::random();
    let muter_user = PubkyAppUser {
        bio: Some("test_homeserver_mute_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:Mute:Muter".to_string(),
        status: None,
    };
    let muter_id = test.create_user(&muter_kp, &muter_user).await?;

    // Create second user (mutee)
    let mutee_kp = Keypair::random();
    let mutee_user = PubkyAppUser {
        bio: Some("test_homeserver_mute_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:Mute:Mutee".to_string(),
        status: None,
    };
    let mutee_id = test.create_user(&mutee_kp, &mutee_user).await?;

    // Mute the user
    test.create_mute(&muter_kp, &mutee_id).await?;

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
    test.cleanup_user(&muter_kp).await?;
    test.cleanup_user(&mutee_kp).await?;

    Ok(())
}
