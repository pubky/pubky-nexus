//! Reads, retries and lookups that start before a switch and land after nexus
//! has followed the user: what they bring from the old homeserver, or about
//! it, is applied on top of the new one.

use anyhow::Result;
use nexus_common::models::user::RecheckOutcome;
use pubky::Keypair;

use crate::harness::{post_uri, MigrationTest};

/// A per-user read of A lists its users when it starts and reaches each one
/// in turn, minutes apart on a homeserver with many users. In between, the
/// mover signs up on B and copies, the recheck moves them to B, and the client
/// wipes A once the recheck has confirmed the switch. The read of A still
/// fetches the mover's events when it reaches them, finds the wipe and
/// applies it: the posts are deleted, the profile is tombstoned, and the scout
/// who replied is told the post was deleted. The runner reads homeservers one
/// after another, so B's read comes after and finds the user tombstoned, and
/// the copy on B is never read. Demonstrated. With deleted users kept on the
/// read list, which fixes `wipe.rs`, B's read restores the data, but the scout
/// is still told the post was deleted and then edited (demonstrated by
/// patching the query).
///
/// The bulk stream checks each event's user against their binding; the
/// per-user read never does. It should check, after the fetch, that the user
/// is still bound to the homeserver it reads.
#[tokio_shared_rt::test(shared)]
#[ignore = "a per-user read applies what it fetched from a homeserver the user was switched away \
            from during the read; needs a binding check after the fetch"]
async fn a_read_of_the_old_homeserver_started_before_the_switch_applies_the_wipe() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let (mover, scout) = (Keypair::random(), Keypair::random());

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    let first = t.post(&session_a, "first", None).await?;
    t.post(&session_a, "second", None).await?;
    let device_scout = t.device()?;
    let session_scout = t.signup(&device_scout, &scout, &t.primary, "scout").await?;
    t.index_primary().await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    t.post(&session_scout, "hi", Some(post_uri(&mover, &first)))
        .await?;
    t.index_primary().await?;
    assert_eq!(t.live_posts(&mover).await?, 2);
    let scout_notifications = t.notifications(&scout).await?;

    // A read of A starts and lists the mover.
    let read_a = t.index_external_paused(&t.hs_a, &mover).await?;

    // Meanwhile: sign up on B, copy, recheck, and wipe A once it says Switched.
    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    let tree = t.public_tree(&device_a, &mover).await?;
    t.write_tree(&session_b, &tree).await?;
    assert_eq!(t.recheck(&mover).await?.outcome, RecheckOutcome::Switched);
    t.delete_tree(&session_a, &tree).await?;

    // The read of A reaches the mover; B's read follows in the same cycle.
    read_a.resume().await?;
    t.index_external(&t.hs_b).await?;
    t.index_external(&t.hs_b).await?;

    let end = (
        t.bound_name(&mover).await?,
        t.live_posts(&mover).await?,
        t.profile_state(&mover).await?,
        t.notifications(&scout).await?,
    );
    assert_eq!(
        end,
        ("hs_b", 2, (false, "mover".to_string()), scout_notifications),
        "(binding, live posts, (tombstoned, name), scout's notifications)"
    );
    Ok(())
}

/// The mover replies from A to a post nexus has not indexed yet, so the reply
/// waits in the retry queue with A as the homeserver it came from. The parent
/// is indexed, the mover moves to B with a copy, and the reply is indexed from
/// B. The mover deletes the reply on B and nexus indexes the delete. When the
/// retry runs, it fetches the reply from A, which still holds it, and indexes
/// it again: a reply the user deleted is back, and the scout's reply
/// notification is issued again, as the newest. Demonstrated.
///
/// The window is the retry backoff, up to an hour between attempts. Before
/// content was fetched from the delivering homeserver, the retry fetched
/// through the user's record and found nothing on B. A retry should be
/// dropped, or fetched from the user's homeserver, once the user is bound to
/// another homeserver than the one it came from.
#[tokio_shared_rt::test(shared)]
#[ignore = "a retry fetches from the homeserver its event came from even after the user left it; \
            needs retries of a user bound elsewhere dropped or fetched from where they are now"]
async fn a_retry_queued_on_the_old_homeserver_restores_a_reply_deleted_on_the_new_one() -> Result<()>
{
    let t = MigrationTest::setup().await?;
    let (mover, scout) = (Keypair::random(), Keypair::random());

    // The scout posts on the primary; nexus has not read it yet.
    let device_scout = t.device()?;
    let session_scout = t.signup(&device_scout, &scout, &t.primary, "scout").await?;
    let topic = t.post(&session_scout, "topic", None).await?;

    // The mover replies from A. The parent is unknown, so the reply is queued
    // for a retry, tagged with A.
    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    let reply = t
        .post(&session_a, "a reply", Some(post_uri(&scout, &topic)))
        .await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 0, "waiting for its parent");
    t.index_primary().await?;

    // Move to B. The copy brings the reply, whose parent is indexed by now.
    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    t.write_tree(&session_b, &t.public_tree(&device_a, &mover).await?)
        .await?;
    assert_eq!(t.recheck(&mover).await?.outcome, RecheckOutcome::Switched);
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 1);

    // The mover deletes the reply on B.
    session_b
        .storage()
        .delete(format!("/pub/pubky.app/posts/{reply}").as_str())
        .await?;
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 0);
    let scout_notifications = t.notifications(&scout).await?;

    // The queued retry runs.
    t.retry().await?;
    assert_eq!(
        (t.live_posts(&mover).await?, t.notifications(&scout).await?),
        (0, scout_notifications),
        "(live posts, scout's notifications)"
    );
    Ok(())
}

/// The recheck runs in the API process and the sweep in the watcher process,
/// each with its own pkarr cache. The watcher's cache still holds the record
/// its last sweep saw, and pkarr's `resolve` serves a cached packet even after
/// it has expired, refreshing it in the background. So the first sweep visit
/// after a recheck, one TTL later, reads the old record unless the entry was
/// evicted, and with stale mapping disabled it moves the user back to A, which
/// their record no longer names. The client wiped A once the recheck had
/// confirmed the switch, so the next read of A applies the wipe: the posts are
/// deleted and the user is tombstoned, read nowhere and never looked up again.
/// Demonstrated, with a second client standing for the watcher's cache; the
/// suite runs in seconds, so that cache holds the old packet unexpired, which
/// is the same answer. With stale mapping enabled, the default, the same visit
/// marks the binding stale instead, and the user is indexed nowhere until the
/// visit after.
///
/// The sweep should not move a binding on a record older than the one the
/// binding was set from (the record clock of the plan), or should look again
/// with `resolve_most_recent` before moving it.
#[tokio_shared_rt::test(shared)]
#[ignore = "the watcher's cache-first sweep applies the record its cache held before the API's \
            recheck; needs record timestamps on the binding or a fresh lookup before a move"]
async fn the_watchers_sweep_moves_the_user_back_after_a_recheck_through_the_api() -> Result<()> {
    let t = MigrationTest::setup().await?;
    let mover = Keypair::random();
    let watcher = t.device()?;

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    t.post(&session_a, "first", None).await?;
    t.post(&session_a, "second", None).await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    // A sweep visit before the move leaves the record naming A in the
    // watcher's cache.
    t.resolve_cached(&mover, &watcher).await?;
    assert_eq!(t.bound_name(&mover).await?, "hs_a");

    // The move, the app's recheck through the API, and the wipe once the
    // switch is confirmed.
    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    let tree = t.public_tree(&device_a, &mover).await?;
    t.write_tree(&session_b, &tree).await?;
    assert_eq!(t.recheck(&mover).await?.outcome, RecheckOutcome::Switched);
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&mover).await?, 2);
    t.delete_tree(&session_a, &tree).await?;

    // The watcher's next visit, then a read of each homeserver.
    t.resolve_cached(&mover, &watcher).await?;
    t.index_external(&t.hs_a).await?;
    t.index_external(&t.hs_b).await?;

    let end = (
        t.bound_name(&mover).await?,
        t.live_posts(&mover).await?,
        t.profile_state(&mover).await?,
    );
    assert_eq!(
        end,
        ("hs_b", 2, (false, "mover".to_string())),
        "(binding, live posts, (tombstoned, name))"
    );
    Ok(())
}
