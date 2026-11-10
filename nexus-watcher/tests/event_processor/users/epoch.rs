use super::utils::{find_cached_user_details, find_user_details, find_user_epoch, set_user_epoch};
use crate::event_processor::posts::utils::{pubky_id, short_post, test_user};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::user::Epoch;
use nexus_watcher::events::handlers;
use nexus_watcher::events::inputs::UserInput;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::user_uri_builder;

/// A profile edit never overwrites a stored epoch, and caches it.
///
/// The gate ignores a `pubky.app` profile PUT of a user at `v1`, so the edit goes to the
/// handler directly.
#[tokio_shared_rt::test(shared)]
async fn test_user_edit_keeps_epoch() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user_id = test
        .create_user(&user_kp, &test_user("Watcher:UserEpoch:Put", "epoch"))
        .await?;
    assert_eq!(find_user_epoch(&user_id).await, None);

    // Only a dedicated epoch write may change it
    set_user_epoch(&user_id, "v1").await?;

    let edited = test_user("Watcher:UserEpoch:Put:Edited", "epoch");
    handlers::user::sync_put(
        UserInput::from_v0(edited, user_uri_builder(user_id.clone())),
        pubky_id(&user_id)?,
    )
    .await?;
    assert_eq!(
        find_user_details(&user_id).await?.name,
        "Watcher:UserEpoch:Put:Edited"
    );
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    assert_eq!(find_cached_user_details(&user_id).await?.epoch, Epoch::V1);

    // Back to `v0`, so the gate lets the cleanup through
    set_user_epoch(&user_id, "v0").await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A tombstone written over a user keeps the stored epoch, and caches it.
///
/// The gate ignores a `pubky.app` profile DEL of a user at `v1`, so the delete goes to the
/// handler directly.
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

    handlers::user::del(pubky_id(&user_id)?).await?;

    assert!(find_user_details(&user_id).await?.deleted);
    assert_eq!(find_user_epoch(&user_id).await.as_deref(), Some("v1"));
    assert_eq!(find_cached_user_details(&user_id).await?.epoch, Epoch::V1);

    // Back to `v0`, so the gate lets the cleanup through
    set_user_epoch(&user_id, "v0").await?;
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;
    Ok(())
}
