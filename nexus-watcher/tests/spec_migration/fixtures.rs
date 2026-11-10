//! The neighbourhood every scenario starts from: Alice, who migrates, and Bob, who stays on
//! `pubky.app`, with everything between them that the flip has to carry over.

use anyhow::{ensure, Result};
use pubky_social_specs::legacy_v0::traits::HasPath;
use pubky_social_specs::legacy_v0::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};
use serde_json::{json, Value};

use crate::harness::{
    author_posts, followers, following, has_bookmarked, post_tags, profile, profile_tags,
    served_post, v0_post, v0_post_uri, v0_profile, v0_profile_uri, SpecMigrationTest, User,
};

pub const AVATAR: &[u8] = b"\x89PNG\r\n\x1a\nalice's avatar";
pub const PHOTO: &[u8] = b"\x89PNG\r\n\x1a\na photo of the sea";
pub const LONG_POST: &str = "On migrations\nThey are long and full of copies.";

pub struct Neighbourhood {
    pub alice: User,
    pub bob: User,
    /// Alice's note, which Bob replied to, reposted, tagged and bookmarked.
    pub note: String,
    /// Alice's post with a photo attached, which she tagged herself.
    pub photo_post: String,
    /// Alice's reply to Bob's post.
    pub reply_to_bob: String,
    /// Alice's long post with a title line, which Bob replied to. Its `social/v1` copy is an
    /// article, whose content is an envelope: the one copy whose content differs.
    pub long: String,
    /// Bob's post, which Alice replied to, tagged and bookmarked.
    pub bob_post: String,
    /// Bob's reply to Alice's note.
    pub bob_reply: String,
    /// Bob's repost of Alice's note.
    pub bob_repost: String,
    pub avatar_hash: String,
    pub photo_hash: String,
}

impl Neighbourhood {
    /// Alice on the external homeserver, read per user, and Bob on the primary, read in bulk,
    /// all of it written through `pubky.app` and indexed by nexus, nothing left to retry.
    pub async fn build(test: &SpecMigrationTest) -> Result<Self> {
        let alice = test.user("Alice", &test.external).await?;
        let bob = test.user("Bob", &test.primary).await?;

        let (avatar, avatar_hash) = alice.upload("avatar.png", "image/png", AVATAR).await?;
        let profile = PubkyAppUser {
            image: Some(avatar),
            ..v0_profile("Alice")
        };
        alice
            .put_json(&PubkyAppUser::create_path(), &profile)
            .await?;
        let (photo, photo_hash) = alice.upload("sea.png", "image/png", PHOTO).await?;

        let bob_post = bob.note("Bob: hello").await?;
        let note = alice.note("Alice: hello").await?;
        let photo_post = alice
            .post(&v0_post("Alice: the sea", None, None, Some(vec![photo])))
            .await?;
        let reply_to_bob = alice
            .reply("Alice: hi Bob", &v0_post_uri(&bob, &bob_post))
            .await?;
        let long = alice
            .post(&PubkyAppPost {
                kind: PubkyAppPostKind::Long,
                ..v0_post(LONG_POST, None, None, None)
            })
            .await?;
        bob.reply("Bob: great read", &v0_post_uri(&alice, &long))
            .await?;

        let alice_note = v0_post_uri(&alice, &note);
        let bob_reply = bob.reply("Bob: hi Alice", &alice_note).await?;
        let bob_repost = bob.repost("Bob: look", &alice_note).await?;
        bob.tag(&alice_note, "great").await?;
        bob.tag(&v0_profile_uri(&alice), "friend").await?;
        bob.bookmark(&alice_note).await?;
        bob.follow(&alice).await?;

        alice.follow(&bob).await?;
        alice.tag(&v0_post_uri(&bob, &bob_post), "nice").await?;
        alice.tag(&v0_post_uri(&alice, &photo_post), "mine").await?;
        alice.tag(&v0_profile_uri(&bob), "pal").await?;
        alice.bookmark(&v0_post_uri(&bob, &bob_post)).await?;

        // Bob's events can reach nexus before Alice's on another homeserver: the retry
        // processor links them once hers are in
        test.index().await?;
        test.retry().await?;
        let queued = test.queued().await?;
        ensure!(queued.is_empty(), "left in the retry queue: {queued:?}");

        Ok(Self {
            alice,
            bob,
            note,
            photo_post,
            reply_to_bob,
            long,
            bob_post,
            bob_reply,
            bob_repost,
            avatar_hash,
            photo_hash,
        })
    }

    /// Alice's posts, oldest first.
    pub fn alice_posts(&self) -> [&str; 4] {
        [&self.note, &self.photo_post, &self.reply_to_bob, &self.long]
    }

    /// What nexus serves about the two of them, leaving out every address and the long post's
    /// content: the flip moves Alice's addresses and nothing else, so this is the same before
    /// and after it.
    pub async fn served(&self) -> Result<Value> {
        let (alice, bob) = (&self.alice, &self.bob);
        let alice_view = profile(alice).await?;
        let bob_view = profile(bob).await?;
        let mut posts = Vec::new();
        for (author, id) in self.alice_posts().into_iter().map(|id| (alice, id)).chain([
            (bob, self.bob_post.as_str()),
            (bob, self.bob_reply.as_str()),
            (bob, self.bob_repost.as_str()),
        ]) {
            let view = served_post(author, id).await?;
            // Which content a migrated long post should have, as `pubky.app` wrote it or as its
            // `social/v1` article copy carries it, is open; the suite does not pin either
            let content = (id != self.long).then_some(view.details.content);
            posts.push(json!({
                "id": id,
                "content": content,
                "kind": view.details.kind,
                "deleted": view.details.deleted,
                "attachments": view.details.attachments.len(),
                "counts": view.counts,
                "tags": post_tags(author, id).await?,
                "replied": view.relationships.replied.is_some(),
                "reposted": view.relationships.reposted.is_some(),
            }));
        }
        Ok(json!({
            "alice": {
                "name": alice_view.details.name,
                "bio": alice_view.details.bio,
                "has_image": alice_view.details.image.is_some(),
                "deleted": alice_view.details.deleted,
                "counts": alice_view.counts,
                "tags": profile_tags(alice).await?,
                "followers": followers(alice).await?,
                "following": following(alice).await?,
                "posts": author_posts(alice).await?,
            },
            "bob": {
                "counts": bob_view.counts,
                "tags": profile_tags(bob).await?,
                "followers": followers(bob).await?,
                "following": following(bob).await?,
                "posts": author_posts(bob).await?,
            },
            "posts": posts,
            "bob_bookmarked_alices_note": has_bookmarked(bob, alice, &self.note).await?,
            "alice_bookmarked_bobs_post": has_bookmarked(alice, bob, &self.bob_post).await?,
        }))
    }
}
