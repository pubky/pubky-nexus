use super::utils::{assert_user_ingested, create_external_test_homeserver};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;

#[tokio_shared_rt::test(shared)]
async fn test_follow_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let followee_hs_pk = create_external_test_homeserver(&mut test).await?;

    let followee_kp = Keypair::random();
    let followee_id = followee_kp.public_key().to_z32();

    test.register_user_in_hs(&followee_kp, &followee_hs_pk)
        .await?;

    let follower_kp = Keypair::random();
    let follower_user = PubkyAppUser {
        bio: Some("test_follow_on_unknown_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:UserIngestion:Follow".to_string(),
        status: None,
    };
    let _follower_id = test.create_user(&follower_kp, &follower_user).await?;

    test.create_follow(&follower_kp, &followee_id).await?;

    assert_user_ingested(&followee_id, &followee_hs_pk).await;

    Ok(())
}
