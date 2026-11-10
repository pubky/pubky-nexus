//! Everything else that happens while the app copies: another device still on `pubky.app`, a
//! flip that fails, a profile that lands before the copies.

use anyhow::{ensure, Result};
use nexus_common::models::user::Epoch;

use crate::fixtures::Neighbourhood;
use crate::harness::{
    author_posts, epoch, notifications, post, post_tags, served_post, v0_post_path, v0_post_uri,
    v1_file_uri, v1_post_uri, v1_profile_uri, SpecMigrationTest,
};
use crate::migrator::{ensure_clean, Pass};

/// Another device of Alice's, still on `pubky.app`, posts after the app listed her tree and
/// before the profile copy. Nexus indexes the post while Alice is `v0`, and the flip moves it
/// to its `social/v1` address with the rest, so nexus serves everything Alice wrote.
///
/// Reasoned, not asserted: the run listed her tree before the post, so her `social/v1` tree has
/// no copy of it, and the flag stops every later run before it lists again. Nexus serves the
/// post at an address her homeserver does not hold until a `rescan`.
#[tokio_shared_rt::test(shared)]
async fn a_pubky_app_post_written_during_the_copy_is_migrated_by_nexus() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;

    let mut migration = n.alice.start_migration().await?;
    migration.run_through(Pass::Posts).await?;
    let late = n.alice.note("Alice: from the old phone").await?;
    test.index().await?;
    ensure_clean(&migration.finish().await?)?;
    test.index().await?;

    assert_eq!(epoch(&n.alice).await?, Epoch::V1);
    let served = served_post(&n.alice, &late).await?;
    assert_eq!(served.details.uri, v1_post_uri(&n.alice, &late));
    assert!(author_posts(&n.alice).await?.contains(&late));
    Ok(())
}

/// Another device deletes a post on `pubky.app` after the app copied it and before the profile
/// copy. Nexus reads the delete while Alice is `v0` and no flip of hers is queued, so it
/// deletes the post, and the flip does not bring it back.
///
/// Reasoned, not asserted: the copy passed its re-check before the delete, so Alice's
/// `social/v1` tree keeps the post that nexus no longer serves.
#[tokio_shared_rt::test(shared)]
async fn a_pubky_app_delete_during_the_copy_holds_after_the_flip() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;

    let mut migration = n.alice.start_migration().await?;
    migration.run_through(Pass::Posts).await?;
    n.alice.delete_post(&n.quiet).await?;
    ensure_clean(&migration.finish().await?)?;
    test.index().await?;

    assert_eq!(epoch(&n.alice).await?, Epoch::V1);
    assert!(post(&n.alice, &n.quiet, None).await?.is_none());
    assert!(!author_posts(&n.alice).await?.contains(&n.quiet));
    assert!(
        test.queued().await?.is_empty(),
        "{:?}",
        test.queued().await?
    );
    Ok(())
}

/// The flip fails: nexus cannot write Alice's files to their `social/v1` addresses. Alice stays
/// `v0`, served as before, and the flip waits in the retry queue. While it waits, her new
/// `social/v1` post and her old device's `pubky.app` delete wait with it, and Bob's tag on her
/// note lands. Once the disk accepts writes again, the retry flips her, then indexes the new
/// post and drops the delete, as any `pubky.app` event of a `v1` user; Bob's tag survives.
#[tokio_shared_rt::test(shared)]
async fn a_failed_flip_holds_the_users_writes_until_it_lands() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;
    let served = n.served().await?;
    test.refuse_file_writes(true)?;

    ensure_clean(&n.alice.migrate().await?)?;
    let fresh = n
        .alice
        .v1_note("Alice: first post from the new app")
        .await?;
    n.alice.delete_post(&n.quiet).await?;
    n.bob
        .tag(&v0_post_uri(&n.alice, &n.note), "patient")
        .await?;
    test.index().await?;

    assert_eq!(epoch(&n.alice).await?, Epoch::V0, "the flip failed");
    let queued = test.queued().await?;
    assert!(queued.contains(&v1_profile_uri(&n.alice)), "{queued:?}");
    assert_eq!(
        queued.len(),
        3,
        "the flip, the new post, the delete: {queued:?}"
    );
    assert!(post(&n.alice, &fresh, None).await?.is_none());
    let quiet = served_post(&n.alice, &n.quiet).await?;
    assert_eq!(quiet.details.uri, v0_post_uri(&n.alice, &n.quiet));
    let tags = post_tags(&n.alice, &n.note).await?;
    assert!(tags.iter().any(|(label, _)| label == "patient"), "{tags:?}");

    test.refuse_file_writes(false)?;
    retry_until_empty(&test).await?;

    assert_eq!(epoch(&n.alice).await?, Epoch::V1);
    let served_fresh = served_post(&n.alice, &fresh).await?;
    assert_eq!(served_fresh.details.uri, v1_post_uri(&n.alice, &fresh));
    let quiet = served_post(&n.alice, &n.quiet).await?;
    assert_eq!(quiet.details.uri, v1_post_uri(&n.alice, &n.quiet));
    let tags = post_tags(&n.alice, &n.note).await?;
    assert!(tags.iter().any(|(label, _)| label == "patient"), "{tags:?}");
    let note = served_post(&n.alice, &n.note).await?;
    assert_eq!(note.details.uri, v1_post_uri(&n.alice, &n.note));
    assert_eq!(
        served["alice"]["posts"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0)
            + 1,
        author_posts(&n.alice).await?.len(),
        "her posts and the new one"
    );
    Ok(())
}

/// Alice's new app writes her `social/v1` profile before the migration copies anything, as an
/// app that signs her in before it migrates does. The profile alone flips her, and the flip
/// moves everything nexus holds. The copies land afterwards, as a `v1` user's writes of
/// objects nexus already holds in the same form: nothing changes and nobody is notified. Her
/// long post is left out of the run, since its copy is no such object: see
/// [`an_article_copied_after_the_flip_is_no_edit`].
#[tokio_shared_rt::test(shared)]
async fn copies_landing_after_the_flip_change_nothing() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;
    let served = n.served().await?;

    let avatar = v1_file_uri(&n.alice, &n.avatar_hash, "png");
    n.alice
        .v1_profile_with_image("Alice", Some(avatar.clone()))
        .await?;
    test.index().await?;
    assert_eq!(epoch(&n.alice).await?, Epoch::V1);
    assert_eq!(n.served().await?, served, "the flip");
    let alice_notified = notifications(&n.alice).await?;
    let bob_notified = notifications(&n.bob).await?;

    let mut migration = n.alice.start_migration().await?;
    migration.fail_reading(&v0_post_path(&n.long));
    let report = migration.finish().await?;
    ensure!(
        report
            .written
            .iter()
            .any(|path| path.starts_with("pub/social/v1/posts/")),
        "the run copies the posts: {report:?}"
    );
    test.index().await?;

    assert_eq!(notifications(&n.alice).await?, alice_notified);
    assert_eq!(notifications(&n.bob).await?, bob_notified);
    assert_eq!(n.served().await?, served, "the copies");
    assert!(
        test.queued().await?.is_empty(),
        "{:?}",
        test.queued().await?
    );
    Ok(())
}

/// The engine's run cannot read Alice's long post and goes on: it writes her profile, which
/// flips her, and ends `incomplete`. Her app runs it again and copies the post, now a `v1`
/// user's write. The copy of a `pubky.app` long post is a `social/v1` article, whose content is
/// an envelope (`{"title":…,"body":…}`, the title taken from the first line), while the flip
/// left the post's content as `pubky.app` wrote it. Nobody edited the post, so nexus should
/// serve it as before and notify nobody.
///
/// Demonstrated: nexus takes the copy for an edit. It replaces the content with the envelope
/// and notifies Bob, who replied to the post, that Alice edited it. The same happens to any
/// copy that reaches nexus after the flip: a profile written before the copies, a run that
/// resumes after an I/O error. A note's copy carries its content unchanged and stays silent.
#[tokio_shared_rt::test(shared)]
#[ignore = "needs the flip to give a long post the content its social/v1 copy carries, or the \
            post handler to tell a migrated post's copy from an edit"]
async fn an_article_copied_after_the_flip_is_no_edit() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;
    let served = n.served().await?;

    let mut migration = n.alice.start_migration().await?;
    migration.fail_reading(&v0_post_path(&n.long));
    let first = migration.finish().await?;
    ensure!(first.incomplete(), "{first:?}");
    test.index().await?;
    assert_eq!(
        epoch(&n.alice).await?,
        Epoch::V1,
        "the profile copy flips her"
    );
    assert_eq!(n.served().await?, served, "the flip");
    let alice_notified = notifications(&n.alice).await?;
    let bob_notified = notifications(&n.bob).await?;

    let second = n.alice.migrate().await?;
    ensure_clean(&second)?;
    ensure!(
        second.written == [format!("pub/social/v1/posts/{0}/{0}.json", n.long)],
        "the second run copies the long post only: {second:?}"
    );
    test.index().await?;

    assert_eq!(notifications(&n.alice).await?, alice_notified);
    assert_eq!(
        notifications(&n.bob).await?,
        bob_notified,
        "nobody edited it"
    );
    assert_eq!(n.served().await?, served);
    Ok(())
}

/// Retry passes until the queue is empty: a deferred event can come up before the flip it
/// waits for, and waits again.
async fn retry_until_empty(test: &SpecMigrationTest) -> Result<()> {
    for _ in 0..4 {
        test.retry().await?;
        if test.queued().await?.is_empty() {
            return Ok(());
        }
    }
    anyhow::bail!("still queued: {:?}", test.queued().await?)
}
