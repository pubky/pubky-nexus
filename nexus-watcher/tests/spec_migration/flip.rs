//! The migration as the app runs it, start to end, and what nexus serves after it.

use anyhow::Result;
use nexus_common::models::user::Epoch;

use crate::fixtures::{Neighbourhood, AVATAR, PHOTO};
use crate::harness::{
    epoch, followers, has_bookmarked, notifications, post_tags, profile, served_post, v0_post_uri,
    v1_file_uri, v1_post_uri, SpecMigrationTest,
};
use crate::migrator::ensure_clean;

/// Alice migrates and nexus reads the whole copy at once. Nexus serves the same profile, posts,
/// counts, tags, follows and bookmarks as before, hers included, and every notification as
/// before, body and time: nobody is notified of anything. Alice's addresses are her
/// `social/v1` ones, her files included and served from there; Bob's reply and repost point at
/// her note's new address, his own posts keep theirs.
#[tokio_shared_rt::test(shared)]
async fn a_migrated_user_keeps_everything() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test).await?;
    let served = n.served().await?;
    let alice_notified = notifications(&n.alice).await?;
    let bob_notified = notifications(&n.bob).await?;
    assert_eq!(epoch(&n.alice).await?, Epoch::V0);
    // The neighbourhood is all there before the flip
    let note = served_post(&n.alice, &n.note).await?;
    assert_eq!((note.counts.replies, note.counts.reposts), (1, 1));
    assert_eq!(
        post_tags(&n.alice, &n.note).await?,
        [("great".to_string(), vec![n.bob.id()])]
    );
    assert_eq!(profile(&n.alice).await?.counts.posts, 4);
    assert!(followers(&n.alice).await?.contains(&n.bob.id()));
    assert!(has_bookmarked(&n.alice, &n.bob, &n.bob_post).await?);
    assert_eq!(profile(&n.alice).await?.counts.bookmarks, 1);
    assert!(alice_notified.len() >= 5, "{alice_notified:?}");

    let report = n.alice.migrate().await?;
    ensure_clean(&report)?;
    test.index().await?;

    assert_eq!(
        epoch(&n.alice).await?,
        Epoch::V1,
        "the profile copy flips Alice"
    );
    assert_eq!(
        n.served().await?,
        served,
        "the flip changes nothing nexus serves"
    );
    for id in n.alice_posts() {
        let view = served_post(&n.alice, id).await?;
        assert_eq!(view.details.uri, v1_post_uri(&n.alice, id), "post {id}");
    }
    let photo = served_post(&n.alice, &n.photo_post).await?;
    assert_eq!(
        photo.details.attachments,
        vec![v1_file_uri(&n.alice, &n.photo_hash, "png")]
    );
    assert_eq!(
        profile(&n.alice).await?.details.image,
        Some(v1_file_uri(&n.alice, &n.avatar_hash, "png"))
    );
    assert_eq!(
        test.served_file(&n.alice, &n.photo_hash, "png").as_deref(),
        Some(PHOTO),
        "the flip copies the photo to its new address"
    );
    assert_eq!(
        test.served_file(&n.alice, &n.avatar_hash, "png").as_deref(),
        Some(AVATAR)
    );
    let reply = served_post(&n.bob, &n.bob_reply).await?;
    assert_eq!(
        reply.relationships.replied,
        Some(v1_post_uri(&n.alice, &n.note))
    );
    let repost = served_post(&n.bob, &n.bob_repost).await?;
    assert_eq!(
        repost.relationships.reposted,
        Some(v1_post_uri(&n.alice, &n.note))
    );
    let alice_reply = served_post(&n.alice, &n.reply_to_bob).await?;
    assert_eq!(
        alice_reply.relationships.replied,
        Some(v0_post_uri(&n.bob, &n.bob_post)),
        "Bob's post keeps its address"
    );
    assert_eq!(notifications(&n.alice).await?, alice_notified);
    assert_eq!(notifications(&n.bob).await?, bob_notified);
    assert!(
        test.queued().await?.is_empty(),
        "{:?}",
        test.queued().await?
    );
    Ok(())
}
