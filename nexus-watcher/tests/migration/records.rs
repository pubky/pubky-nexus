//! Records that name no reachable homeserver.

use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::PubkyId;

use crate::harness::MigrationTest;

/// Opting out is a switch to a key that answers nothing: the edge follows it,
/// reads there fail and index nothing, and the data already indexed stays.
/// Naming the real homeserver again resumes from the stored cursor.
#[tokio_shared_rt::test(shared)]
async fn opting_out_to_a_dead_key_and_returning() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    t.recheck(&mover).await?;
    let cursor_a = t.cursor(&mover, &t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 1);

    let dead = Keypair::random().public_key();
    t.switch_record(&device_a, &mover, &dead).await?;
    t.recheck(&mover).await?;
    assert_eq!(
        t.bound_to(&mover).await?.as_deref(),
        Some(dead.to_z32().as_str())
    );

    // Reading the dead homeserver fails and changes nothing; reading A no
    // longer covers the user.
    t.post(&session_a, "second, unseen", None).await?;
    assert!(t.index_external(&PubkyId::from(dead)).await.is_err());
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.cursor(&mover, &t.hs_a).await?, cursor_a);
    assert_eq!(t.live_posts(&mover).await?, 1);
    assert_eq!(t.profile_state(&mover).await?, (false, "mover".to_string()));

    // Back on the record: the read resumes where it stopped.
    t.switch_record(&device_a, &mover, &t.hs_a.to_public_key())
        .await?;
    t.recheck(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_a.as_ref()));
    t.index_external(&t.hs_a).await?;
    assert!(t.cursor(&mover, &t.hs_a).await? > cursor_a);
    assert_eq!(t.live_posts(&mover).await?, 2);
    Ok(())
}

/// A record with no `_pubky` entry says nothing about where the data is: the
/// binding stands and the user keeps being read there, and a later record
/// naming another homeserver is still followed.
#[tokio_shared_rt::test(shared)]
async fn record_without_homeserver_keeps_the_binding() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    t.recheck(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_a.as_ref()));

    t.publish_without_homeserver(&device_a, &mover).await?;
    t.recheck(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_a.as_ref()));
    t.post(&session_a, "second, still on A", None).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(
        t.live_posts(&mover).await?,
        2,
        "the user is still read on A"
    );

    t.switch_record(&device_a, &mover, &t.hs_b.to_public_key())
        .await?;
    t.recheck(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));
    Ok(())
}
