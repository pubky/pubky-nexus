//! Switches to and from the primary homeserver, whose stream is bulk-indexed
//! with one global cursor.

use anyhow::Result;
use pubky::Keypair;

use crate::harness::MigrationTest;

/// External to primary. The bulk stream skips the user's events while the
/// edge still names the old homeserver, so after the switch the user's own
/// history on the primary has to be read per user before the bulk stream
/// covers the user.
#[tokio_shared_rt::test(shared)]
async fn external_to_primary_catches_up_then_the_bulk_stream_takes_over() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.post(&session_a, "second", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);

    // Sign up on the primary and copy. The bulk stream runs before nexus
    // notices the switch and passes over the copy as another homeserver's.
    let device_p = t.device()?;
    let session_p = t.signup(&device_p, &mover, &t.primary, "mover").await?;
    t.write_tree(&session_p, &t.public_tree(&device_a, &mover).await?)
        .await?;
    t.index_primary().await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_a.as_ref()));
    assert_eq!(t.cursor(&mover, &t.primary).await?, 0);

    t.recheck(&mover).await?;
    assert_eq!(
        t.bound_to(&mover).await?.as_deref(),
        Some(t.primary.as_ref())
    );

    // The bulk stream has nothing left for the user; the per-user read does.
    t.index_primary().await?;
    assert_eq!(t.cursor(&mover, &t.primary).await?, 0);
    t.catch_up().await?;
    assert!(t.cursor(&mover, &t.primary).await? > 0);
    assert_eq!(
        t.live_posts(&mover).await?,
        2,
        "the copy is not indexed twice"
    );

    // From here the bulk stream alone covers the user.
    t.post(&session_p, "third, on the primary", None).await?;
    t.index_primary().await?;
    assert_eq!(t.live_posts(&mover).await?, 3);
    Ok(())
}

/// Primary to external and back. The return reads the user's own history on
/// the primary again from their per-user cursor there, which the idempotent
/// handlers make safe; a cursor floor stored when leaving would make that read
/// shorter and is a later optimisation.
#[tokio_shared_rt::test(shared)]
async fn primary_to_external_and_back() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_p = t.device()?;
    let session_p = t.signup(&device_p, &mover, &t.primary, "mover").await?;
    t.post(&session_p, "first", None).await?;
    t.post(&session_p, "second", None).await?;
    t.index_primary().await?;
    t.recheck(&mover).await?;
    assert_eq!(
        t.bound_to(&mover).await?.as_deref(),
        Some(t.primary.as_ref())
    );
    assert_eq!(t.live_posts(&mover).await?, 2);

    // Leave for A.
    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.write_tree(&session_a, &t.public_tree(&device_p, &mover).await?)
        .await?;
    let bulk_cursor = t.bulk_cursor().await?;
    assert!(bulk_cursor > 0);
    t.recheck(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_a.as_ref()));
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);
    t.post(&session_a, "third, on A", None).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 3);

    // Return: copy the new post to the primary through the old device, whose
    // cache still names the primary, and publish the record.
    let session_p = t.session(&device_p, &mover).await?;
    t.write_tree(&session_p, &t.public_tree(&device_a, &mover).await?)
        .await?;
    t.switch_record(&device_p, &mover, &t.primary.to_public_key())
        .await?;
    t.index_primary().await?;
    assert_eq!(
        t.live_posts(&mover).await?,
        3,
        "skipped as another homeserver's"
    );

    t.recheck(&mover).await?;
    assert_eq!(
        t.bound_to(&mover).await?.as_deref(),
        Some(t.primary.as_ref())
    );
    t.catch_up().await?;
    assert!(t.cursor(&mover, &t.primary).await? > bulk_cursor);
    assert_eq!(t.live_posts(&mover).await?, 3);

    t.post(&session_p, "fourth, home again", None).await?;
    t.index_primary().await?;
    assert_eq!(t.live_posts(&mover).await?, 4);
    Ok(())
}
