use crate::event_processor::{
    homeserver::utils::create_external_test_homeserver, utils::watcher::WatcherTest,
};
use anyhow::Result;
use nexus_common::{db::PubkyClient, models::homeserver::Homeserver};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyId};

#[tokio_shared_rt::test(shared)]
async fn test_follow_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a separate homeserver for the followee
    let followee_hs_pk = create_external_test_homeserver(&mut test).await?;
    let followe_hs_id = PubkyId::try_from(&followee_hs_pk.to_z32()).unwrap();

    // Create followee
    let followee_kp = Keypair::random();
    let followee_id = followee_kp.public_key().to_z32();

    // Register the tagged post author PK in the new homeserver
    // We only need the record mapping, not necessarily the profile.json being uploaded
    PubkyClient::get()?
        .signup(&followee_kp, &followee_hs_pk, None)
        .await?;

    // Create tagger user
    let follower_kp = Keypair::random();

    let follower_user = PubkyAppUser {
        bio: Some("test_follow_on_unknown_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:Homeserver:Follow".to_string(),
        status: None,
    };
    let follower_id = test
        .create_user(&follower_kp, &follower_user)
        .await
        .unwrap();

    // Follow the followee
    test.create_follow(&follower_id, &followee_id).await?;

    assert!(Homeserver::get_by_id(followe_hs_id)
        .await
        .unwrap()
        .is_some());

    Ok(())
}
