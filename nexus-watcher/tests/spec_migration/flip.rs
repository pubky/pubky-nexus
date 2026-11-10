//! The migration as the app runs it, start to end, and what nexus serves after it.

use anyhow::Result;
use nexus_common::models::user::Epoch;
use pubky_social_specs::PubkyId;

use crate::fixtures::{Neighbourhood, AVATAR, PHOTO};
use crate::harness::{
    epoch, followers, notifications, post_tags, profile, served_post, v0_post, v0_post_uri,
    v1_file_uri, v1_post_uri, SpecMigrationTest,
};
use crate::migrator::{ensure_clean, Pass};
use pubky_social_specs::migrate::TRANSFORM_REV;

/// Alice migrates on `alice_hs` and nexus reads the whole copy at once. Nexus serves the same
/// profile, posts, counts, tags, follows and bookmark as before; Alice's addresses are her
/// `social/v1` ones, her files included and served from there; Bob's reply and repost point at her note's new
/// address; nobody is notified of anything.
async fn a_migrated_user_keeps_everything(
    alice_hs: fn(&SpecMigrationTest) -> PubkyId,
) -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &alice_hs(&test)).await?;
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
    assert_eq!(profile(&n.alice).await?.counts.posts, 5);
    assert!(followers(&n.alice).await?.contains(&n.bob.id()));
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

#[tokio_shared_rt::test(shared)]
async fn a_migrated_user_on_the_primary_keeps_everything() -> Result<()> {
    a_migrated_user_keeps_everything(|test| test.primary.clone()).await
}

#[tokio_shared_rt::test(shared)]
async fn a_migrated_user_on_an_external_homeserver_keeps_everything() -> Result<()> {
    a_migrated_user_keeps_everything(|test| test.external.clone()).await
}

/// Nexus reads after every pass of the copy. The copies of a `v0` user are ignored, so nothing
/// changes, not even an address, until the profile copy flips her; then everything moves at
/// once.
#[tokio_shared_rt::test(shared)]
async fn nexus_reading_between_the_passes_sees_nothing_until_the_profile() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;
    let served = n.served().await?;
    let bob_notified = notifications(&n.bob).await?;

    let mut migration = n.alice.start_migration().await?;
    for pass in [
        Pass::Files,
        Pass::Blobs,
        Pass::Posts,
        Pass::Tags,
        Pass::Follows,
    ] {
        migration.run_through(pass).await?;
        test.index().await?;
        assert_eq!(epoch(&n.alice).await?, Epoch::V0, "after {pass:?}");
        assert_eq!(n.served().await?, served, "after {pass:?}");
        let note = served_post(&n.alice, &n.note).await?;
        assert_eq!(
            note.details.uri,
            v0_post_uri(&n.alice, &n.note),
            "after {pass:?}"
        );
    }
    let report = migration.finish().await?;
    ensure_clean(&report)?;
    test.index().await?;

    assert_eq!(epoch(&n.alice).await?, Epoch::V1);
    assert_eq!(n.served().await?, served);
    let note = served_post(&n.alice, &n.note).await?;
    assert_eq!(note.details.uri, v1_post_uri(&n.alice, &n.note));
    assert_eq!(notifications(&n.bob).await?, bob_notified);
    Ok(())
}

/// The app runs the migration again, as it does on every start until it sees the flag. A
/// check of the port's flag and journal alone, which the other scenarios rely on: the test
/// never lets nexus read after the neighbourhood is built.
///
/// - The flag at the current revision stops the run before it lists anything.
/// - A flag of an older revision (new transforms) walks the tree again. Every object the flag
///   records is present and the File objects are read again, so a post another device wrote
///   since, with the old photo attached, is copied with the photo's `social/v1` address.
/// - With no flag at all (lost), the journal finds every key present and writes nothing.
#[tokio_shared_rt::test(shared)]
async fn a_second_run_writes_nothing() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;
    let first = n.alice.migrate().await?;
    ensure_clean(&first)?;
    let flag_path = "/priv/social/v1/_migrated.json";
    let flag = n.alice.get_json(flag_path).await?;

    let again = n.alice.migrate().await?;
    assert!(again.already_migrated, "{again:?}");

    let late = n
        .alice
        .post(&v0_post(
            "Alice: the sea again",
            None,
            None,
            Some(vec![n.photo.clone()]),
        ))
        .await?;
    let mut older = flag.clone();
    older["transform_rev"] = (TRANSFORM_REV - 1).into();
    n.alice.put_json(flag_path, &older).await?;
    let revision = n.alice.migrate().await?;
    let copy = format!("pub/social/v1/posts/{late}/{late}.json");
    assert_eq!(
        revision.written,
        std::slice::from_ref(&copy),
        "{revision:?}"
    );
    assert!(!revision.already_present.is_empty(), "{revision:?}");
    let copied = n.alice.get_json(&format!("/{copy}")).await?;
    assert_eq!(
        copied["attachments"][0]["uri"],
        v1_file_uri(&n.alice, &n.photo_hash, "png")
    );
    let rewritten = n.alice.get_json(flag_path).await?;
    assert_eq!(rewritten["transform_rev"], TRANSFORM_REV);
    let migrated = rewritten["migrated"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for path in flag["migrated"].as_array().into_iter().flatten() {
        assert!(migrated.contains(path), "{path} is recorded again");
    }

    n.alice.delete(flag_path).await?;
    let lost = n.alice.migrate().await?;
    assert!(lost.written.is_empty(), "{lost:?}");
    assert!(!lost.already_present.is_empty(), "{lost:?}");
    Ok(())
}
