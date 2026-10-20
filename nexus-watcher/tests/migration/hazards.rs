//! Attempts to break a migration: devices and records doing what the happy
//! path does not expect.

use std::sync::Arc;

use anyhow::Result;
use nexus_common::models::error::ModelError;
use nexus_common::models::user::{PubkyConnectorResolver, RecheckOutcome, UserIngestor};
use pubky::Keypair;
use pubky_app_specs::PubkyId;

use crate::harness::MigrationTest;

/// A device whose cache still names the old homeserver keeps writing there
/// after the switch. Nexus follows the record, so those writes are indexed
/// only if the user ever names that homeserver again.
#[tokio_shared_rt::test(shared)]
async fn writes_to_the_homeserver_the_record_left_wait_for_a_return() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;

    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    t.write_tree(&session_b, &t.public_tree(&device_a, &mover).await?)
        .await?;
    t.recheck(&mover).await?;
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 1);

    // The old device still resolves A from its cache and posts there.
    t.post(&session_a, "second, on A after the switch", None)
        .await?;
    t.index_external(&t.hs_a).await?;
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 1, "A is not read any more");

    // Only a return to A brings it in, from the cursor where A was left.
    t.switch_record(&device_a, &mover, &t.hs_a.to_public_key())
        .await?;
    t.recheck(&mover).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);
    Ok(())
}

/// The record names a real homeserver where the user never signed up. The
/// switch is followed, the read there answers 404 and backs the user off,
/// what was indexed stays, and a later signup there is picked up.
#[tokio_shared_rt::test(shared)]
async fn record_naming_a_homeserver_without_an_account() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;

    t.switch_record(&device_a, &mover, &t.hs_b.to_public_key())
        .await?;
    assert_eq!(t.recheck(&mover).await?.outcome, RecheckOutcome::Switched);
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.cursor(&mover, &t.hs_b).await?, 0);
    assert_eq!(t.live_posts(&mover).await?, 1);
    assert_eq!(t.profile_state(&mover).await?, (false, "mover".to_string()));

    // The account appears later. The 404 backoff skips the user for a few
    // reads, then the copy is indexed.
    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    t.write_tree(&session_b, &t.public_tree(&device_a, &mover).await?)
        .await?;
    t.post(&session_b, "second, on B", None).await?;
    let mut reads = 0;
    while t.cursor(&mover, &t.hs_b).await? == 0 && reads < 8 {
        t.index_external(&t.hs_b).await?;
        reads += 1;
    }
    assert!(
        t.cursor(&mover, &t.hs_b).await? > 0,
        "read within {reads} runs"
    );
    assert_eq!(t.live_posts(&mover).await?, 2);
    Ok(())
}

/// The copy on B is partly deleted again before nexus reads B: the PUT finds
/// nothing to fetch and the DEL removes the post. The end state is what B
/// holds.
#[tokio_shared_rt::test(shared)]
async fn copy_then_delete_on_the_new_homeserver_before_the_first_read() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    let first = t.post(&session_a, "first", None).await?;
    t.post(&session_a, "second", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);

    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    t.write_tree(&session_b, &t.public_tree(&device_a, &mover).await?)
        .await?;
    session_b
        .storage()
        .delete(format!("/pub/pubky.app/posts/{first}").as_str())
        .await?;

    t.recheck(&mover).await?;
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 1);
    Ok(())
}

/// Anyone can call the recheck for anyone. For a user whose record did not
/// change it confirms the binding and changes nothing else.
#[tokio_shared_rt::test(shared)]
async fn recheck_by_a_stranger_changes_nothing() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    let before = t.binding(&mover).await?;
    let cursor_a = t.cursor(&mover, &t.hs_a).await?;

    for _ in 0..3 {
        let recheck = t.recheck(&mover).await?;
        assert_eq!(recheck.outcome, RecheckOutcome::Confirmed);
        assert_eq!(recheck.homeserver.as_deref(), Some(t.hs_a.as_ref()));
    }
    assert_eq!(t.binding(&mover).await?, before);
    assert_eq!(t.cursor(&mover, &t.hs_a).await?, cursor_a);
    assert_eq!(t.live_posts(&mover).await?, 1);
    Ok(())
}

/// A copy to the primary plus a post written only there, both before nexus
/// notices the switch: the bulk stream passed over them as another
/// homeserver's events and nothing reads them afterwards. The copy is
/// harmless, the post is lost.
#[tokio_shared_rt::test(shared)]
#[ignore = "the primary's bulk stream skipped the user's events and nothing re-reads them; \
            needs the per-user catch-up read of the primary"]
async fn switch_to_the_primary_without_a_catch_up_loses_what_the_bulk_stream_skipped() -> Result<()>
{
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;

    let device_p = t.device()?;
    let session_p = t.signup(&device_p, &mover, &t.primary, "mover").await?;
    t.write_tree(&session_p, &t.public_tree(&device_a, &mover).await?)
        .await?;
    t.post(&session_p, "second, only on the primary", None)
        .await?;
    t.index_primary().await?;

    t.recheck(&mover).await?;
    t.index_primary().await?;
    t.index_primary().await?;
    assert_eq!(t.live_posts(&mover).await?, 2);
    Ok(())
}

/// A record naming a blacklisted homeserver: the recheck refuses and leaves
/// the binding where it was, so the user keeps being read on the old
/// homeserver and nothing from the blacklisted one is ever fetched.
#[tokio_shared_rt::test(shared)]
async fn recheck_refuses_a_blacklisted_homeserver() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    let before = t.binding(&mover).await?;

    let device_b = t.device()?;
    t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    let blacklisting = UserIngestor::new([t.hs_b.clone()], Arc::new(PubkyConnectorResolver));
    let err = blacklisting
        .recheck_user(&PubkyId::from(mover.public_key()))
        .await
        .expect_err("a blacklisted homeserver is refused");
    assert!(matches!(err, ModelError::HsBlacklisted { ref hs_id } if *hs_id == *t.hs_b));
    assert_eq!(t.binding(&mover).await?, before);
    assert_eq!(t.live_posts(&mover).await?, 1);
    Ok(())
}

/// A user nexus knows only as a stub, ingested because someone referenced
/// them and not yet read on their homeserver, can be rechecked too.
#[tokio_shared_rt::test(shared)]
async fn recheck_works_for_a_user_known_only_as_a_stub() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.ingest(&mover).await?;

    let recheck = t.recheck(&mover).await?;
    assert_eq!(recheck.outcome, RecheckOutcome::Confirmed);
    assert_eq!(recheck.homeserver.as_deref(), Some(t.hs_a.as_ref()));

    let device_b = t.device()?;
    t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    assert_eq!(t.recheck(&mover).await?.outcome, RecheckOutcome::Switched);
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));
    Ok(())
}
