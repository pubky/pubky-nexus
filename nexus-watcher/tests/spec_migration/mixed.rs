//! A network with both epochs: a migrated user and the users who have not migrated keep
//! reaching each other, each by the addresses their own app writes.

use anyhow::Result;
use nexus_common::models::user::Epoch;
use serde_json::{json, Value};

use crate::fixtures::Neighbourhood;
use crate::harness::{
    epoch, followers, notifications, post_tags, profile_tags, served_post, v0_post_uri,
    v0_profile_uri, v1_post_uri, v1_profile_uri, SpecMigrationTest, User,
};
use crate::migrator::ensure_clean;

/// After Alice's flip, each side keeps writing the addresses its own app knows, and each write
/// lands on the post or profile it names, with its notification. A reply is served with its
/// parent at the parent's own address, whichever spelling the reply wrote.
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
    let alice_notified = notifications(&n.alice).await?.len();
    let bob_notified = notifications(&n.bob).await?.len();

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

    let tagged = |label: &str, tagger: &str| (label.to_string(), vec![tagger.to_string()]);
    let (alice, bob) = (n.alice.id(), n.bob.id());

    // Bob reaches Alice. A post's parent is served at the parent's own address, her new one.
    let after = served_post(&n.alice, &n.note).await?;
    assert_eq!(after.counts.replies, note.counts.replies + 1);
    let reply = served_post(&n.bob, &bob_reply).await?;
    assert_eq!(
        reply.relationships.replied,
        Some(v1_post_uri(&n.alice, &n.note))
    );
    assert_eq!(
        post_tags(&n.alice, &n.note).await?,
        [tagged("classic", &bob), tagged("great", &bob)]
    );
    assert_eq!(
        profile_tags(&n.alice).await?,
        [tagged("friend", &bob), tagged("neighbour", &bob)]
    );
    // Alice reaches Bob, whose post keeps its pubky.app address
    let after = served_post(&n.bob, &n.bob_post).await?;
    assert_eq!(after.counts.replies, bob_post.counts.replies + 1);
    let reply = served_post(&n.alice, &alice_reply).await?;
    assert_eq!(
        reply.relationships.replied,
        Some(v0_post_uri(&n.bob, &n.bob_post))
    );
    assert_eq!(
        post_tags(&n.bob, &n.bob_post).await?,
        [tagged("evergreen", &alice), tagged("nice", &alice)]
    );
    assert_eq!(
        profile_tags(&n.bob).await?,
        [tagged("neighbour", &alice), tagged("pal", &alice)]
    );
    // Carol and Alice follow each other
    assert!(followers(&n.alice).await?.contains(&carol.id()));
    assert!(followers(&carol).await?.contains(&alice));

    // A reply notification names the parent at its stored address, a tag notification the
    // target as the tagger wrote it: Bob's is the social/v1 spelling Alice's app wrote, which
    // his homeserver does not hold
    assert_eq!(
        new_notifications(&n.alice, alice_notified).await?,
        sorted(vec![
            json!({"type": "follow", "followed_by": carol.id()}),
            json!({"type": "tag_profile", "tag_label": "neighbour", "tagged_by": bob}),
            json!({"type": "tag_post", "tag_label": "classic", "tagged_by": bob,
                   "post_uri": old_note, "post_kind": "short"}),
            json!({"type": "reply", "replied_by": bob, "post_kind": "short",
                   "parent_post_uri": v1_post_uri(&n.alice, &n.note),
                   "reply_uri": v0_post_uri(&n.bob, &bob_reply)}),
        ])
    );
    assert_eq!(
        new_notifications(&n.bob, bob_notified).await?,
        sorted(vec![
            json!({"type": "tag_profile", "tag_label": "neighbour", "tagged_by": alice}),
            json!({"type": "tag_post", "tag_label": "evergreen", "tagged_by": alice,
                   "post_uri": bob_post_v1, "post_kind": "short"}),
            json!({"type": "reply", "replied_by": alice, "post_kind": "short",
                   "parent_post_uri": v0_post_uri(&n.bob, &n.bob_post),
                   "reply_uri": v1_post_uri(&n.alice, &alice_reply)}),
        ])
    );
    assert_eq!(
        new_notifications(&carol, 0).await?,
        [json!({"type": "new_friend", "followed_by": alice})]
    );
    let queued = test.queued().await?;
    assert!(queued.is_empty(), "{queued:?}");
    Ok(())
}

/// The bodies of `user`'s notifications newer than the first `before` ones they had, sorted.
async fn new_notifications(user: &User, before: usize) -> Result<Vec<Value>> {
    let mut all = notifications(user).await?;
    all.truncate(all.len() - before);
    Ok(sorted(all.into_iter().map(|(_, body)| body).collect()))
}

fn sorted(mut bodies: Vec<Value>) -> Vec<Value> {
    bodies.sort_by_key(Value::to_string);
    bodies
}
