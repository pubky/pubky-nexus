use super::utils::{find_user_details, find_user_epoch, set_user_epoch};
use crate::event_processor::posts::utils::{short_post, test_user};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{user_uri_builder, PubkyAppTag};

/// A profile PUT creates the user with `epoch` `v0`, and an edit never overwrites a stored epoch.
#[tokio_shared_rt::test(shared)]
async fn test_user_put_is_born_v0_and_edit_keeps_epoch() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:UserEpoch:Put", "epoch"))
        .await?;
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v0"));

    // Only a dedicated epoch write may change it
    set_user_epoch(&user_id, "v1").await?;

    test.create_profile(
        &user_kp,
        &test_user("Watcher:UserEpoch:Put:Edited", "epoch"),
    )
    .await?;
    assert_eq!(
        find_user_details(&user_id).await?.name,
        "Watcher:UserEpoch:Put:Edited"
    );
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A tombstone written over a user keeps the stored epoch.
#[tokio_shared_rt::test(shared)]
async fn test_user_tombstone_keeps_epoch() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:UserEpoch:Tombstone", "epoch"))
        .await?;
    let (_post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:UserEpoch:Tombstone:Post"))
        .await?;
    // Stands in for the flip
    set_user_epoch(&user_id, "v1").await?;

    test.cleanup_user(&user_kp).await?;

    assert!(find_user_details(&user_id).await?.deleted);
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));

    test.cleanup_post(&user_kp, &post_path).await?;
    Ok(())
}

/// A stub user created by key discovery is born `v0`.
#[tokio_shared_rt::test(shared)]
async fn test_stub_user_is_born_v0() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Signed up on the homeserver, but no profile.json
    let stub_kp = Keypair::random();
    let stub_id = stub_kp.public_key().to_z32();
    test.register_user(&stub_kp).await?;

    let tagger_kp = Keypair::random();
    test.create_user(
        &tagger_kp,
        &test_user("Watcher:UserEpoch:Stub:Tagger", "epoch"),
    )
    .await?;

    // Tagging an unknown user ingests it as a stub
    let tag = PubkyAppTag {
        uri: user_uri_builder(stub_id.clone()),
        label: "epoch-user-stub".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    assert_eq!(find_user_epoch(&stub_id).await.as_deref(), Some("v0"));

    test.del(&tagger_kp, &tag_path).await?;
    test.cleanup_user(&tagger_kp).await?;
    Ok(())
}
