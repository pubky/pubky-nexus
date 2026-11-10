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

/// After Alice's flip, each side keeps writing the addresses its own app knows, and each write
/// lands on the post or profile it names, with its notification.
///
/// - Bob's `pubky.app` writes Alice's old addresses: a reply to her note, a tag on it, a tag on
///   her profile.
/// - Alice's `social/v1` app writes the `social/v1` spelling of Bob's addresses, the one the
///   migrator rewrites every reference to: a reply to his post, a tag on it, a tag on his
///   profile.
/// - Carol, new to both, follows Alice from `pubky.app`, and Alice follows Carol from
///   `social/v1`.
#[tokio_shared_rt::test(shared)]
async fn both_epochs_keep_reaching_each_other() -> Result<()> {
    let test = SpecMigrationTest::setup().await?;
    let n = Neighbourhood::build(&test).await?;
    let carol = test.user("Carol", &test.primary).await?;
    ensure_clean(&n.alice.migrate().await?)?;
    test.index().await?;
    assert_eq!(epoch(&n.alice).await?, Epoch::V1);
    let note = served_post(&n.alice, &n.note).await?;
    let bob_post = served_post(&n.bob, &n.bob_post).await?;
    let alice_notified = notification_kinds(&n.alice).await?.len();
    let bob_notified = notification_kinds(&n.bob).await?.len();

    let old_note = v0_post_uri(&n.alice, &n.note);
    let bob_reply = n.bob.reply("Bob: still here", &old_note).await?;
    n.bob.tag(&old_note, "classic").await?;
    n.bob.tag(&v0_profile_uri(&n.alice), "neighbour").await?;
    let bob_post_v1 = v1_post_uri(&n.bob, &n.bob_post);
    let alice_reply = n.alice.v1_reply("Alice: still here", &bob_post_v1).await?;
    n.alice.v1_tag(&bob_post_v1, "evergreen").await?;
    n.alice.v1_tag(&v1_profile_uri(&n.bob), "neighbour").await?;
    carol.follow(&n.alice).await?;
    n.alice.v1_follow(&carol).await?;
    test.index().await?;
    test.retry().await?;

    // Bob reaches Alice
    let after = served_post(&n.alice, &n.note).await?;
    assert_eq!(after.counts.replies, note.counts.replies + 1);
    let reply = served_post(&n.bob, &bob_reply).await?;
    assert!(reply.relationships.replied.is_some());
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
    // Alice reaches Bob
    let after = served_post(&n.bob, &n.bob_post).await?;
    assert_eq!(after.counts.replies, bob_post.counts.replies + 1);
    let reply = served_post(&n.alice, &alice_reply).await?;
    assert!(reply.relationships.replied.is_some());
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
    // Carol and Alice follow each other
    assert!(followers(&n.alice).await?.contains(&carol.id()));
    assert!(followers(&carol).await?.contains(&n.alice.id()));

    let new = |mut kinds: Vec<String>, before: usize| {
        kinds.truncate(kinds.len() - before);
        kinds.sort();
        kinds
    };
    assert_eq!(
        new(notification_kinds(&n.alice).await?, alice_notified),
        ["follow", "reply", "tag_post", "tag_profile"]
    );
    assert_eq!(
        new(notification_kinds(&n.bob).await?, bob_notified),
        ["reply", "tag_post", "tag_profile"]
    );
    assert_eq!(notification_kinds(&carol).await?, ["new_friend"]);
    let queued = test.queued().await?;
    assert!(queued.is_empty(), "{queued:?}");
    Ok(())
}
