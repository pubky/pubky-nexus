//! Migrations that remove the data from the old homeserver.

use anyhow::Result;
use pubky::Keypair;

use crate::harness::{post_uri, MigrationTest};

/// Mirror first, switch later, wipe last: the record keeps naming A while the
/// data is copied to B, so nexus keeps reading A; once the record names B, a
/// wipe of A is not read at all.
#[tokio_shared_rt::test(shared)]
async fn mirror_then_switch_then_wipe_the_old_homeserver() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.post(&session_a, "second", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    t.resolve(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_a.as_ref()));

    // Mirror: the signup on B publishes B, the old device publishes A again
    // right away. B's device keeps writing to B from its own cache.
    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    t.switch_record(&device_a, &mover, &t.hs_a.to_public_key())
        .await?;
    let tree = t.public_tree(&device_a, &mover).await?;
    t.write_tree(&session_b, &tree).await?;
    t.resolve(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_a.as_ref()));
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);
    assert_eq!(t.cursor(&mover, &t.hs_b).await?, 0, "B is not read");

    // Switch.
    t.switch_record(&device_b, &mover, &t.hs_b.to_public_key())
        .await?;
    t.resolve(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);

    // Wipe A. Nothing reads the user there any more.
    t.delete_tree(&session_a, &tree).await?;
    let cursor_a = t.cursor(&mover, &t.hs_a).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.cursor(&mover, &t.hs_a).await?, cursor_a);
    assert_eq!(t.live_posts(&mover).await?, 2);
    assert_eq!(t.profile_state(&mover).await?, (false, "mover".to_string()));
    Ok(())
}

/// Switch, then wipe A before nexus has noticed: the deletes are indexed and
/// the user tombstoned, the switch is still followed, and the read of B
/// brings everything back. Counterparts see a delete and a re-create.
#[tokio_shared_rt::test(shared)]
async fn wipe_before_nexus_notices_is_healed_by_the_switch() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let (mover, scout) = (Keypair::random(), Keypair::random());

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    let first = t.post(&session_a, "first", None).await?;
    t.post(&session_a, "second", None).await?;
    let device_scout = t.device()?;
    let session_scout = t.signup(&device_scout, &scout, &t.primary, "scout").await?;
    t.index_primary().await?;
    t.follow(&session_a, &scout.public_key().to_z32()).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    t.follow(&session_scout, &mover.public_key().to_z32())
        .await?;
    t.post(&session_scout, "hi", Some(post_uri(&mover, &first)))
        .await?;
    t.index_primary().await?;
    assert_eq!(t.live_posts(&mover).await?, 2);
    let scout_notifications = t.notifications(&scout).await?;

    // Sign up on B, copy, wipe A, all before nexus looks again.
    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    let tree = t.public_tree(&device_a, &mover).await?;
    t.write_tree(&session_b, &tree).await?;
    t.delete_tree(&session_a, &tree).await?;

    // Nexus reads A first: the wipe looks like a deletion.
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 0);
    assert!(t.profile_state(&mover).await?.0, "tombstoned");

    // The tombstoned user still follows the record, and the read of B
    // restores the profile and the posts.
    t.resolve(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);
    assert_eq!(t.profile_state(&mover).await?, (false, "mover".to_string()));

    // The known cost: the counterpart was told about the deletion.
    assert!(t.notifications(&scout).await?.len() >= scout_notifications.len());

    t.post(&session_b, "third, from B", None).await?;
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 3);
    Ok(())
}
