//! Switches between two external homeservers.

use anyhow::Result;
use nexus_common::models::user::RecheckOutcome;
use pubky::Keypair;

use crate::harness::{post_uri, MigrationTest};

/// Copy and switch from one external homeserver to another, with nexus
/// noticing the switch before the copy has even started: the first read of
/// the new homeserver finds only the profile, later reads pick up the rest.
/// Nothing is indexed twice and nobody gets a second notification.
#[tokio_shared_rt::test(shared)]
async fn copy_and_switch_noticed_before_the_copy() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let (mover, scout) = (Keypair::random(), Keypair::random());

    // The mover lives on A with two posts and follows the scout; the scout,
    // on the primary, follows back, replies and tags.
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
    t.tag(&session_scout, post_uri(&mover, &first), "nice")
        .await?;
    t.index_primary().await?;

    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_a.as_ref()));
    assert_eq!(t.live_posts(&mover).await?, 2);
    let mover_notifications = t.notification_times(&mover).await?;
    let scout_notifications = t.notification_times(&scout).await?;
    assert_eq!(mover_notifications.len(), 3, "follow, reply and tag");
    assert!(!scout_notifications.is_empty(), "the mover's follow");

    // Signing up on B publishes the record naming B. Nexus notices at once
    // and follows the user to B.
    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    let recheck = t.recheck(&mover).await?;
    assert_eq!(recheck.outcome, RecheckOutcome::Switched);
    assert_eq!(recheck.homeserver.as_deref(), Some(t.hs_b.as_ref()));
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));
    assert_eq!(
        t.recheck(&mover).await?.outcome,
        RecheckOutcome::Confirmed,
        "a second look changes nothing"
    );
    t.index_external(&t.hs_b).await?;
    assert_eq!(
        t.live_posts(&mover).await?,
        2,
        "nothing is removed on a switch"
    );

    // The copy lands on B after the first read; the next read continues from
    // the stored cursor.
    let tree = t.public_tree(&device_a, &mover).await?;
    assert_eq!(tree.len(), 4, "profile, two posts, one follow");
    t.write_tree(&session_b, &tree).await?;
    let cursor_before = t.cursor(&mover, &t.hs_b).await?;
    t.index_external(&t.hs_b).await?;
    assert!(t.cursor(&mover, &t.hs_b).await? > cursor_before);

    assert_eq!(t.live_posts(&mover).await?, 2);
    // Same entries, same times: nothing was notified again.
    assert_eq!(t.notification_times(&mover).await?, mover_notifications);
    assert_eq!(t.notification_times(&scout).await?, scout_notifications);

    // Life goes on from B alone.
    t.post(&session_b, "third, from B", None).await?;
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 3);
    Ok(())
}

/// The same migration with nexus noticing only after the copy is complete,
/// through the periodic resolver with stale mapping disabled instead of a
/// recheck.
#[tokio_shared_rt::test(shared)]
async fn copy_and_switch_noticed_after_the_copy() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.post(&session_a, "second", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);

    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    let tree = t.public_tree(&device_a, &mover).await?;
    t.write_tree(&session_b, &tree).await?;

    // Reads of A in the meantime change nothing: the copy happened on B.
    let cursor_a = t.cursor(&mover, &t.hs_a).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.cursor(&mover, &t.hs_a).await?, cursor_a);

    t.resolve(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));
    t.index_external(&t.hs_b).await?;
    assert!(t.cursor(&mover, &t.hs_b).await? > 0, "B has been read");
    assert_eq!(t.live_posts(&mover).await?, 2);
    assert_eq!(t.profile_state(&mover).await?, (false, "mover".to_string()));
    Ok(())
}

/// A user switching back and forth keeps a cursor per homeserver, so a
/// return reads only what is new there.
#[tokio_shared_rt::test(shared)]
async fn switching_back_and_forth_keeps_a_cursor_per_homeserver() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    let cursor_a = t.cursor(&mover, &t.hs_a).await?;
    assert!(cursor_a > 0);

    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    t.write_tree(&session_b, &t.public_tree(&device_a, &mover).await?)
        .await?;
    t.recheck(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));
    t.index_external(&t.hs_b).await?;
    let cursor_b = t.cursor(&mover, &t.hs_b).await?;
    assert!(cursor_b > 0);
    assert_eq!(t.live_posts(&mover).await?, 1);

    // Back to A: the read finds nothing new there, both cursors stand.
    t.switch_record(&device_a, &mover, &t.hs_a.to_public_key())
        .await?;
    t.recheck(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_a.as_ref()));
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.cursor(&mover, &t.hs_a).await?, cursor_a);
    assert_eq!(t.cursor(&mover, &t.hs_b).await?, cursor_b);

    // A post written on A in the meantime is picked up from the cursor.
    t.post(&session_a, "second, on A again", None).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);

    // And to B once more.
    t.switch_record(&device_b, &mover, &t.hs_b.to_public_key())
        .await?;
    t.recheck(&mover).await?;
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));
    assert_eq!(t.cursor(&mover, &t.hs_b).await?, cursor_b);
    assert_eq!(t.live_posts(&mover).await?, 2);
    Ok(())
}

/// A user who signs up elsewhere without copying anything: nexus keeps what
/// it indexed from the old homeserver and follows the new one from there.
#[tokio_shared_rt::test(shared)]
async fn fresh_signup_without_a_copy_keeps_the_old_index() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "old name").await?;
    t.post(&session_a, "first", None).await?;
    t.post(&session_a, "second", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);

    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "new name").await?;
    t.post(&session_b, "third, only on B", None).await?;
    t.recheck(&mover).await?;
    t.index_external(&t.hs_b).await?;

    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));
    assert_eq!(t.live_posts(&mover).await?, 3);
    assert_eq!(
        t.profile_state(&mover).await?,
        (false, "new name".to_string())
    );
    Ok(())
}
