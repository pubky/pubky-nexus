use super::utils::{
    find_user_details, find_user_epoch, remove_user_epoch, set_user_epoch, user_node_exists,
};
use crate::event_processor::posts::utils::{short_post, test_user};
use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::user::{Epoch, UserEpoch};
use pubky::{Keypair, ResourcePath};
use pubky_app_specs::traits::{HasIdPath, HashId};
use pubky_app_specs::{user_uri_builder, PubkyAppMute, PubkyAppTag};

/// Writes a mute of a random user: a pubky.app event whose handler indexes nothing.
async fn put_mute(test: &mut WatcherTest, user_kp: &Keypair) -> Result<ResourcePath> {
    let mutee_id = Keypair::random().public_key().to_z32();
    let mute_path: ResourcePath = PubkyAppMute::create_path(&mutee_id).parse()?;
    test.put(user_kp, &mute_path, PubkyAppMute::new()).await?;
    Ok(mute_path)
}

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
    // Stands in for the flip, which writes the graph and then the index
    set_user_epoch(&user_id, "v1").await?;
    UserEpoch::put_to_index(&user_id, Epoch::V1).await?;

    test.cleanup_user(&user_kp).await?;

    assert!(find_user_details(&user_id).await?.deleted);
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, Some(Epoch::V1));

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

/// The profile PUT is checked before its handler creates the node, so the
/// user's next pubky.app event is the one that indexes their epoch.
#[tokio_shared_rt::test(shared)]
async fn test_first_events_after_profile_index_epoch() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:UserEpoch:Index", "epoch"))
        .await?;
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, None);

    let (_post_id, post_path) = test
        .create_post(&user_kp, &short_post("Watcher:UserEpoch:Index:Post"))
        .await?;
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v0"));
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, Some(Epoch::V0));

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A missing index field is refilled from the node's stored epoch.
#[tokio_shared_rt::test(shared)]
async fn test_index_miss_refills_from_graph() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:UserEpoch:Refill", "epoch"))
        .await?;
    let mute_path = put_mute(&mut test, &user_kp).await?;
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, Some(Epoch::V0));

    set_user_epoch(&user_id, "v1").await?;
    UserEpoch::del_from_index(&user_id).await?;

    test.del(&user_kp, &mute_path).await?;
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, Some(Epoch::V1));

    test.cleanup_user(&user_kp).await?;
    UserEpoch::del_from_index(&user_id).await?;
    Ok(())
}

/// A node written without an epoch gets `v0` in the graph before the index.
#[tokio_shared_rt::test(shared)]
async fn test_index_miss_initialises_missing_node_epoch() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:UserEpoch:Init", "epoch"))
        .await?;
    remove_user_epoch(&user_id).await?;
    assert_eq!(find_user_epoch(&user_id).await, None);

    put_mute(&mut test, &user_kp).await?;
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v0"));
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, Some(Epoch::V0));

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Another app's tag folder doesn't index its owner's epoch.
#[tokio_shared_rt::test(shared)]
async fn test_other_app_tag_skips_index() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:UserEpoch:Mapky", "epoch"))
        .await?;

    let tag = PubkyAppTag {
        uri: "https://example.com/epoch-mapky".to_string(),
        label: "epoch-mapky".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path: ResourcePath = format!("/pub/mapky/tags/{}", tag.create_id()).parse()?;
    test.put(&user_kp, &tag_path, &tag).await?;
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, None);

    test.del(&user_kp, &tag_path).await?;
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, None);

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// An event from a user with no `User` node yet writes no epoch, creates no
/// node and still succeeds.
#[tokio_shared_rt::test(shared)]
async fn test_event_before_user_node_writes_nothing() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Signed up on the homeserver, but no profile.json
    let user_kp = Keypair::random();
    let user_id = user_kp.public_key().to_z32();
    test.register_user(&user_kp).await?;

    let mute_path = put_mute(&mut test, &user_kp).await?;

    let event_line = format!("PUT pubky://{user_id}{}", mute_path.as_str());
    let event_handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(&event_line, event_handler).await?;

    assert!(!user_node_exists(&user_id).await);
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, None);

    Ok(())
}

/// A hard delete removes the user's index field along with the node.
#[tokio_shared_rt::test(shared)]
async fn test_hard_delete_removes_index_field() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:UserEpoch:HardDelete", "epoch"),
        )
        .await?;
    put_mute(&mut test, &user_kp).await?;
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, Some(Epoch::V0));

    test.cleanup_user(&user_kp).await?;
    assert!(!user_node_exists(&user_id).await);
    assert_eq!(UserEpoch::get_from_index(&user_id).await?, None);

    Ok(())
}
