//! Life after the flip: the `social/v1` app writes, an old device still writes `pubky.app`.

use anyhow::Result;
use nexus_common::models::user::Epoch;
use pubky_social_specs::legacy_v0::traits::HasPath;
use pubky_social_specs::legacy_v0::PubkyAppUser;

use crate::fixtures::Neighbourhood;
use crate::harness::{
    author_posts, epoch, notification_kinds, notifications, post, post_tags, profile, served_post,
    v0_profile, v1_post_uri, SpecMigrationTest,
};
use crate::migrator::ensure_clean;

/// Alice and Bob as the flip leaves them: Alice migrated and read.
async fn migrated() -> Result<(SpecMigrationTest, Neighbourhood)> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;
    ensure_clean(&n.alice.migrate().await?)?;
    test.index().await?;
    assert_eq!(epoch(&n.alice).await?, Epoch::V1);
    Ok((test, n))
}

/// The `social/v1` app picks up where `pubky.app` left off: a new post, an edit of a migrated
/// post, the delete of another, a profile edit and the delete of a tag the migrator copied
/// all land, on top of what the flip carried. Bob, who replied to and reposted the note, is
/// notified of the edit and the untag and of nothing else; the profile edit is no second flip.
#[tokio_shared_rt::test(shared)]
async fn the_v1_app_writes_on_top_of_what_the_flip_carried() -> Result<()> {
    let (test, n) = migrated().await?;
    let posts_before = profile(&n.alice).await?.counts.posts;
    let bob_notified = notifications(&n.bob).await?.len();

    let fresh = n
        .alice
        .v1_note("Alice: first post from the new app")
        .await?;
    // A migrated post's only version is its creation, whose edit id is the post id
    n.alice
        .v1_edit(&n.note, &n.note, "Alice: hello, edited")
        .await?;
    n.alice.v1_delete_post(&n.quiet).await?;
    n.alice.v1_profile("Alice, migrated").await?;
    // The migrator wrote her tag on Bob's post as a social/v1 tag file; the app deletes it
    n.alice
        .v1_untag(&v1_post_uri(&n.bob, &n.bob_post), "nice")
        .await?;
    test.index().await?;

    let served = served_post(&n.alice, &fresh).await?;
    assert_eq!(served.details.uri, v1_post_uri(&n.alice, &fresh));
    assert_eq!(
        served_post(&n.alice, &n.note).await?.details.content,
        "Alice: hello, edited"
    );
    assert!(
        post(&n.alice, &n.quiet, None).await?.is_none(),
        "the deleted post is gone"
    );
    let posts = author_posts(&n.alice).await?;
    assert!(
        posts.contains(&fresh) && !posts.contains(&n.quiet),
        "{posts:?}"
    );
    let alice = profile(&n.alice).await?;
    assert_eq!(alice.details.name, "Alice, migrated");
    assert_eq!(alice.details.epoch, Epoch::V1);
    assert_eq!(alice.counts.posts, posts_before, "one post in, one out");
    let tags = post_tags(&n.bob, &n.bob_post).await?;
    assert!(tags.iter().all(|(label, _)| label != "nice"), "{tags:?}");
    let bob_kinds = notification_kinds(&n.bob).await?;
    let mut new = bob_kinds[..bob_kinds.len() - bob_notified].to_vec();
    new.sort();
    new.dedup();
    assert_eq!(new, ["post_edited", "untag_post"], "{bob_kinds:?}");
    assert!(
        test.queued().await?.is_empty(),
        "{:?}",
        test.queued().await?
    );
    Ok(())
}

/// A device that still runs `pubky.app` keeps writing there after the flip: a post, a delete of
/// a migrated post, a profile. Nexus reads Alice through `social/v1` only, so none of it lands:
/// the post is not indexed, the deleted post stays, the profile keeps its `social/v1` name.
/// The writes are not queued either, so they never land later.
#[tokio_shared_rt::test(shared)]
async fn an_old_device_writing_pubky_app_after_the_flip_is_not_read() -> Result<()> {
    let (test, n) = migrated().await?;
    let served = n.served().await?;

    let late = n.alice.note("Alice: from the old phone").await?;
    n.alice.delete_post(&n.note).await?;
    n.alice
        .put_json(
            &PubkyAppUser::create_path(),
            &v0_profile("Alice on the old phone"),
        )
        .await?;
    test.index().await?;

    assert!(post(&n.alice, &late, None).await?.is_none());
    assert_eq!(n.served().await?, served);
    assert!(
        test.queued().await?.is_empty(),
        "{:?}",
        test.queued().await?
    );
    Ok(())
}
