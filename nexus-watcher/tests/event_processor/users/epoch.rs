use super::utils::{find_user_details, find_user_epoch, set_user_epoch};
use crate::event_processor::posts::utils::{short_post, test_user};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;

/// A profile edit never overwrites a stored epoch.
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
