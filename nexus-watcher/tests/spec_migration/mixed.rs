//! A network with both epochs: a migrated user and the users who have not migrated keep
//! reaching each other, each by the addresses their own app writes.

use anyhow::Result;
use nexus_common::models::user::Epoch;

use crate::fixtures::Neighbourhood;
use crate::harness::{
    epoch, followers, notification_kinds, post_tags, profile_tags, served_post, v0_post_uri,
    v0_profile_uri, v1_post_uri, v1_profile_uri, SpecMigrationTest,
};
use crate::migrator::ensure_clean;

/// Bob's `pubky.app` keeps writing Alice's old addresses after her flip: a reply to her note, a
/// tag on it, a tag on her profile, and Carol, new to both, follows her. Each lands on Alice's
/// migrated post and profile, and Alice is notified of each.
#[tokio_shared_rt::test(shared)]
async fn an_unmigrated_user_reaches_a_migrated_one_by_her_old_addresses() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;
    let carol = test.user("Carol", &test.primary).await?;
    ensure_clean(&n.alice.migrate().await?)?;
    test.index().await?;
    assert_eq!(epoch(&n.alice).await?, Epoch::V1);
    let note = served_post(&n.alice, &n.note).await?;
    let alice_notified = notification_kinds(&n.alice).await?.len();

    let old_note = v0_post_uri(&n.alice, &n.note);
    let reply = n.bob.reply("Bob: still here", &old_note).await?;
    n.bob.tag(&old_note, "classic").await?;
    n.bob.tag(&v0_profile_uri(&n.alice), "neighbour").await?;
    carol.follow(&n.alice).await?;
    test.index().await?;
    test.retry().await?;

    let after = served_post(&n.alice, &n.note).await?;
    assert_eq!(after.counts.replies, note.counts.replies + 1);
    assert!(served_post(&n.bob, &reply)
        .await?
        .relationships
        .replied
        .is_some());
    let tags = post_tags(&n.alice, &n.note).await?;
    assert!(
        tags.contains(&("classic".to_string(), vec![n.bob.id()])),
        "{tags:?}"
    );
    let tags = profile_tags(&n.alice).await?;
    assert!(
        tags.contains(&("neighbour".to_string(), vec![n.bob.id()])),
        "{tags:?}"
    );
    assert!(followers(&n.alice).await?.contains(&carol.id()));
    let mut new: Vec<String> = notification_kinds(&n.alice).await?;
    new.truncate(new.len() - alice_notified);
    new.sort();
    assert_eq!(new, ["follow", "reply", "tag_post", "tag_profile"]);
    assert!(
        test.queued().await?.is_empty(),
        "{:?}",
        test.queued().await?
    );
    Ok(())
}

/// Alice's `social/v1` app reaches Bob, who has not migrated, by the `social/v1` spelling of
/// his addresses, the one the migrator rewrites every reference to: a reply to his post, a tag
/// on it, a tag on his profile, and a follow of Carol. Each lands on Bob's `pubky.app` post and
/// profile, and Bob and Carol are notified.
#[tokio_shared_rt::test(shared)]
async fn a_migrated_user_reaches_an_unmigrated_one_by_the_v1_spelling() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test, &test.primary).await?;
    let carol = test.user("Carol", &test.primary).await?;
    ensure_clean(&n.alice.migrate().await?)?;
    test.index().await?;
    let bob_post = served_post(&n.bob, &n.bob_post).await?;
    let bob_notified = notification_kinds(&n.bob).await?.len();

    let bob_post_v1 = v1_post_uri(&n.bob, &n.bob_post);
    let reply = n.alice.v1_reply("Alice: still here", &bob_post_v1).await?;
    n.alice.v1_tag(&bob_post_v1, "evergreen").await?;
    n.alice.v1_tag(&v1_profile_uri(&n.bob), "neighbour").await?;
    n.alice.v1_follow(&carol).await?;
    test.index().await?;
    test.retry().await?;

    let after = served_post(&n.bob, &n.bob_post).await?;
    assert_eq!(after.counts.replies, bob_post.counts.replies + 1);
    assert!(served_post(&n.alice, &reply)
        .await?
        .relationships
        .replied
        .is_some());
    let tags = post_tags(&n.bob, &n.bob_post).await?;
    assert!(
        tags.contains(&("evergreen".to_string(), vec![n.alice.id()])),
        "{tags:?}"
    );
    let tags = profile_tags(&n.bob).await?;
    assert!(
        tags.contains(&("neighbour".to_string(), vec![n.alice.id()])),
        "{tags:?}"
    );
    assert!(followers(&carol).await?.contains(&n.alice.id()));
    let mut new: Vec<String> = notification_kinds(&n.bob).await?;
    new.truncate(new.len() - bob_notified);
    new.sort();
    assert_eq!(new, ["reply", "tag_post", "tag_profile"]);
    assert_eq!(notification_kinds(&carol).await?, ["follow"]);
    assert!(
        test.queued().await?.is_empty(),
        "{:?}",
        test.queued().await?
    );
    Ok(())
}
