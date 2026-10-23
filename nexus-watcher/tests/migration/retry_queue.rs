//! A switch while the retry queue still holds the user's events from the old
//! homeserver.

use anyhow::{anyhow, Result};
use nexus_common::models::post::PostDetails;
use pubky::Keypair;
use pubky_app_specs::traits::HasIdPath;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind};

use crate::harness::{post_uri, MigrationTest};

struct Queued {
    t: MigrationTest,
    mover: Keypair,
    scout: Keypair,
    session_b: pubky::PubkySession,
    reply_id: String,
    parent_uri: String,
}

/// The mover replies on A to a post nexus has not indexed yet, so the reply
/// is queued for retry with A as its origin. The parent is indexed, then the
/// mover moves to B with a copy and nexus follows.
async fn reply_queued_on_a_then_switch_to_b() -> Result<Queued> {
    let t = MigrationTest::setup().await?;
    let (mover, scout) = (Keypair::random(), Keypair::random());

    let device_scout = t.device()?;
    let session_scout = t.signup(&device_scout, &scout, &t.primary, "scout").await?;
    let parent_id = t.post(&session_scout, "parent", None).await?;
    let parent_uri = post_uri(&scout, &parent_id);

    let device_a = t.device()?;
    let session_a = t.signup(&device_a, &mover, &t.hs_a, "mover").await?;
    let reply_id = t
        .post(
            &session_a,
            "reply as written on A",
            Some(parent_uri.clone()),
        )
        .await?;
    t.ingest(&mover).await?;
    t.index_external(&t.hs_a).await?;
    assert_eq!(t.live_posts(&mover).await?, 0, "the parent is missing");

    t.index_primary().await?;

    let device_b = t.device()?;
    let session_b = t.signup(&device_b, &mover, &t.hs_b, "mover").await?;
    t.write_tree(&session_b, &t.public_tree(&device_a, &mover).await?)
        .await?;
    t.recheck(&mover).await?;
    assert_eq!(t.bound_to(&mover).await?.as_deref(), Some(t.hs_b.as_ref()));

    Ok(Queued {
        t,
        mover,
        scout,
        session_b,
        reply_id,
        parent_uri,
    })
}

async fn content(mover: &Keypair, post_id: &str) -> Result<Option<String>> {
    Ok(
        PostDetails::get_from_index(&mover.public_key().to_z32(), post_id)
            .await
            .map_err(|e| anyhow!("{e}"))?
            .map(|d| d.content),
    )
}

/// The reply is edited on B after the move. The queued retry still fetches
/// from A and must not put the old text back, nor tell the parent's author
/// about an edit the mover never made.
#[tokio_shared_rt::test(shared)]
#[ignore = "the retry processor replays a queued event without asking where the user is bound now; \
            needs the retry to drop or re-route events whose origin is no longer the user's homeserver"]
async fn queued_retry_from_the_old_homeserver_after_an_edit_on_the_new_one() -> Result<()> {
    let q = reply_queued_on_a_then_switch_to_b().await?;
    let t = &q.t;

    let edited = PubkyAppPost::new(
        "reply as edited on B".to_string(),
        PubkyAppPostKind::Short,
        Some(q.parent_uri.clone()),
        None,
        None,
    );
    q.session_b
        .storage()
        .put(
            PubkyAppPost::create_path(&q.reply_id).as_str(),
            serde_json::to_vec(&edited)?,
        )
        .await?;
    t.index_external(&t.hs_b).await?;
    assert_eq!(
        content(&q.mover, &q.reply_id).await?.as_deref(),
        Some("reply as edited on B")
    );
    let scout_notifications = t.notifications(&q.scout).await?;

    let uri = post_uri(&q.mover, &q.reply_id);
    assert!(t.fire_retry(&uri).await?, "the reply is queued from A");

    assert_eq!(
        content(&q.mover, &q.reply_id).await?.as_deref(),
        Some("reply as edited on B"),
        "the retry put A's copy back"
    );
    assert_eq!(
        t.notifications(&q.scout).await?,
        scout_notifications,
        "the parent's author was told about an edit the mover never made"
    );
    Ok(())
}

/// The reply is deleted on B after the move. The queued retry still fetches
/// from A and must not bring it back.
#[tokio_shared_rt::test(shared)]
#[ignore = "the retry processor replays a queued event without asking where the user is bound now; \
            needs the retry to drop or re-route events whose origin is no longer the user's homeserver"]
async fn queued_retry_from_the_old_homeserver_after_a_delete_on_the_new_one() -> Result<()> {
    let q = reply_queued_on_a_then_switch_to_b().await?;
    let t = &q.t;

    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&q.mover).await?, 1, "the copy is indexed");
    q.session_b
        .storage()
        .delete(PubkyAppPost::create_path(&q.reply_id).as_str())
        .await?;
    t.index_external(&t.hs_b).await?;
    assert_eq!(t.live_posts(&q.mover).await?, 0, "deleted on B");

    let uri = post_uri(&q.mover, &q.reply_id);
    assert!(t.fire_retry(&uri).await?, "the reply is queued from A");

    assert_eq!(
        t.live_posts(&q.mover).await?,
        0,
        "the retry resurrected a post deleted on B"
    );
    Ok(())
}
