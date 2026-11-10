//! The engine's copies reaching nexus after the flip, as a `v1` user's writes: a profile
//! written before the copies, a run that resumes after an I/O error.

use anyhow::{ensure, Result};
use nexus_common::models::user::Epoch;

use crate::fixtures::Neighbourhood;
use crate::harness::{
    epoch, notifications, served_post, v0_post_path, v1_file_uri, SpecMigrationTest,
};
use crate::migrator::ensure_clean;

/// Alice's new app writes her `social/v1` profile before the migration copies anything, as an
/// app that signs her in before it migrates does. The profile alone flips her, and the flip
/// moves everything nexus holds. The copies land afterwards, as a `v1` user's writes of
/// objects nexus already holds in the same form: nothing changes and nobody is notified. Her
/// long post is left out of the run, since its copy is no such object: see
/// [`an_article_copied_after_the_flip_is_no_edit`].
#[tokio_shared_rt::test(shared)]
async fn copies_landing_after_the_flip_change_nothing() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test).await?;
    let served = n.served().await?;

    let avatar = v1_file_uri(&n.alice, &n.avatar_hash, "png");
    n.alice.v1_profile("Alice", Some(avatar)).await?;
    test.index().await?;
    assert_eq!(epoch(&n.alice).await?, Epoch::V1);
    assert_eq!(n.served().await?, served, "the flip");
    let alice_notified = notifications(&n.alice).await?;
    let bob_notified = notifications(&n.bob).await?;

    let mut migration = n.alice.start_migration().await?;
    migration.fail_reading(&v0_post_path(&n.long));
    let report = migration.finish().await?;
    // Everything but the profile, which the app wrote, and the long post: her posts, files and
    // follow at the addresses they derive, her three tags and her private bookmark
    let mut copies = report.written.clone();
    copies.sort();
    let (tags, rest): (Vec<_>, Vec<_>) = copies
        .into_iter()
        .partition(|path| path.starts_with("pub/social/v1/tags/"));
    let (bookmarks, rest): (Vec<_>, Vec<_>) = rest
        .into_iter()
        .partition(|path| path.starts_with("priv/social/v1/bookmarks/"));
    let mut expected: Vec<String> = [&n.note, &n.photo_post, &n.reply_to_bob]
        .iter()
        .map(|id| format!("pub/social/v1/posts/{id}/{id}.json"))
        .chain(
            [&n.avatar_hash, &n.photo_hash]
                .iter()
                .map(|hash| format!("pub/social/v1/files/{hash}.png")),
        )
        .chain([format!("pub/social/v1/follows/{}.json", n.bob.id())])
        .collect();
    expected.sort();
    assert_eq!(rest, expected, "{report:?}");
    assert_eq!((tags.len(), bookmarks.len()), (3, 1), "{report:?}");
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
/// left the post's content as `pubky.app` wrote it. Nobody edited the post, so its copy should
/// change nothing: not its content, whichever form the flip gives it, and not anyone's
/// notifications.
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
    let n = Neighbourhood::build(&test).await?;
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
    let content = served_post(&n.alice, &n.long).await?.details.content;
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
    assert_eq!(
        served_post(&n.alice, &n.long).await?.details.content,
        content
    );
    assert_eq!(n.served().await?, served);
    Ok(())
}
