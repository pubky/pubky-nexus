//! The handlers' inputs: one plain struct per resource, holding exactly the fetched data its
//! handler reads, the address the object is stored under, and a [`Reference`] for each address
//! the object holds. The [`ResourceKey`] the event's path names travels beside the input.
//!
//! A translator fills them from a validated spec object, so the handlers never see the spec's
//! object layout, and translation alone decides each object's stored address. A few field types
//! (`PubkyAppPostKind`, `PubkyAppUserLink`) still come from the spec, as they do in the stored
//! models.

use chrono::Utc;
use nexus_common::models::file::{FileDetails, FileUrls};
use nexus_common::models::post::{PostDetails, PostParent, PostParents};
use nexus_common::models::user::{Epoch, UserDetails};
use pubky_social_specs::legacy_v0::{PubkyAppPostKind, PubkyAppUserLink};
use pubky_social_specs::PubkyId;

pub use nexus_common::models::ResourceKey;

/// An address one object holds to another: the address as written and what it names.
///
/// The translator resolves it once (`resolve_reference`, next to the event router), so a handler
/// routes on [`Reference::target`] and never parses the address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// The address exactly as the author wrote it.
    pub uri: String,
    pub target: ReferenceTarget,
}

/// What a [`Reference`] names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceTarget {
    /// A post: its author and post id.
    Post(ResourceKey),
    /// A user's profile.
    User(PubkyId),
    /// A file: its owner and file id.
    File(ResourceKey),
    /// A tag: its tagger and tag id; `app` is set when the tag lives in another app's folder.
    Tag {
        key: ResourceKey,
        app: Option<String>,
    },
    /// Anything else: a URL, another app's object, a resource Nexus never references (a follow,
    /// a bookmark, a blob, …), an address that doesn't parse, or one of an epoch the resolver
    /// doesn't read yet.
    Other,
}

impl ReferenceTarget {
    /// The user who owns what the reference names; `None` for [`ReferenceTarget::Other`].
    pub fn owner_id(&self) -> Option<&PubkyId> {
        match self {
            Self::Post(key) | Self::File(key) | Self::Tag { key, .. } => Some(&key.owner_id),
            Self::User(user_id) => Some(user_id),
            Self::Other => None,
        }
    }
}

/// A post, as the post handler reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct PostInput {
    /// The address the post is stored under.
    pub uri: String,
    pub content: String,
    pub kind: PubkyAppPostKind,
    /// If a reply, the parent the author named.
    pub parent: Option<Reference>,
    /// The embedded object the author named.
    pub embed: Option<Reference>,
    /// A collection post's items, in order; empty for any other kind. Only author ingestion
    /// reads them: the collected edges still come from the stored content.
    pub collection_items: Vec<Reference>,
    pub attachments: Option<Vec<String>>,
    /// `pubky://` URL of the lock server; `None` when the post is unlocked.
    pub lock: Option<String>,
}

impl PostInput {
    pub fn into_details(self, key: &ResourceKey) -> PostDetails {
        PostDetails {
            uri: self.uri,
            content: self.content,
            id: key.resource_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
            author: key.owner_id.to_string(),
            kind: self.kind,
            attachments: self.attachments,
            lock: self.lock,
            deleted: false,
        }
    }

    /// The posts the parent and embed name, each with the address as written.
    ///
    /// The address is not the parent's stored one: the post handler writes the edges from the
    /// keys and reads the stored addresses back from the graph. Only a post parent gives a reply
    /// edge: the post handler rejects a reply to a file, profile or tag, and a reply to anything
    /// else stays a reply with no edge. Only a post can be reposted; other embed targets stay
    /// plain embeds.
    pub fn parents(&self) -> PostParents {
        let post_parent = |reference: &Reference| match &reference.target {
            ReferenceTarget::Post(key) => Some(PostParent {
                uri: reference.uri.clone(),
                key: key.clone(),
            }),
            _ => None,
        };
        PostParents {
            replied: self.parent.as_ref().and_then(post_parent),
            reposted: self.embed.as_ref().and_then(post_parent),
        }
    }
}

/// A profile, as the user handler reads it.
///
/// No `PartialEq`: the v0 `PubkyAppUserLink` it carries has none.
#[derive(Clone, Debug)]
pub struct UserInput {
    /// The address the profile is stored under.
    pub uri: String,
    pub name: String,
    pub bio: Option<String>,
    pub image: Option<String>,
    /// `None` when the profile has no links or `null`; an empty list stays `Some`, so the stored
    /// `null` and `[]` stay distinct.
    pub links: Option<Vec<PubkyAppUserLink>>,
    pub status: Option<String>,
}

impl UserInput {
    /// The details cached for the profile; [`UserInput::uri`] goes to the graph only.
    pub fn into_details(self, user_id: &PubkyId) -> UserDetails {
        UserDetails {
            name: self.name,
            bio: self.bio,
            status: self.status,
            links: self.links,
            image: self.image,
            id: user_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
            deleted: false,
            epoch: Epoch::V0,
        }
    }
}

/// A tag, as the tag handlers and moderation read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagInput {
    /// The address the tag is stored under, kept on its edge.
    pub uri: String,
    /// The tagged object.
    pub tagged: Reference,
    pub label: String,
}

/// A bookmark, as the bookmark handler reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkInput {
    /// The bookmarked object; only a post is indexed.
    pub bookmarked: Reference,
}

/// A file's metadata, as the file handler reads it. The handler downloads the blob from `src`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInput {
    /// The address the file's metadata is stored under.
    pub uri: String,
    pub name: String,
    /// The URI of the blob, as the author wrote it.
    pub src: String,
    pub content_type: String,
    pub size: usize,
    pub created_at: i64,
}

impl FileInput {
    pub fn into_details(self, key: &ResourceKey) -> FileDetails {
        let user_id = key.owner_id.to_string();
        let file_id = key.resource_id.clone();
        FileDetails {
            urls: FileUrls::new(&user_id, &file_id, &self.content_type),
            name: self.name,
            src: self.src,
            content_type: self.content_type,
            uri: self.uri,
            id: file_id,
            created_at: self.created_at,
            indexed_at: Utc::now().timestamp_millis(),
            owner_id: user_id,
            size: self.size as i64,
            metadata: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTHOR: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    const POST_ID: &str = "003286NSMY490";

    fn key() -> ResourceKey {
        ResourceKey {
            owner_id: PubkyId::try_from(AUTHOR).unwrap(),
            resource_id: POST_ID.into(),
        }
    }

    fn reference(address: &str, target: ReferenceTarget) -> Reference {
        Reference {
            uri: address.to_string(),
            target,
        }
    }

    fn post(parent: Option<Reference>, embed: Option<Reference>) -> PostInput {
        PostInput {
            uri: format!("pubky://{AUTHOR}/pub/pubky.app/posts/{POST_ID}"),
            content: "x".into(),
            kind: PubkyAppPostKind::Short,
            parent,
            embed,
            collection_items: Vec::new(),
            attachments: None,
            lock: None,
        }
    }

    #[test]
    fn post_parent_and_embed_become_reply_and_repost() {
        let address = format!("pubky://{AUTHOR}/pub/pubky.app/posts/{POST_ID}");
        let parent = reference(&address, ReferenceTarget::Post(key()));
        let parents = post(Some(parent.clone()), Some(parent)).parents();
        let expected = Some(PostParent {
            uri: address,
            key: key(),
        });
        assert_eq!(parents.replied, expected, "a post parent should be replied");
        assert_eq!(
            parents.reposted, expected,
            "a post embed should be reposted"
        );
    }

    #[test]
    fn non_post_parent_and_embed_give_no_edges() {
        // A file and an unresolved address: neither is a reply nor a repost edge.
        let file = reference(
            "pubky://x/pub/pubky.app/files/1",
            ReferenceTarget::File(key()),
        );
        let other = reference("https://example.com", ReferenceTarget::Other);
        for target in [file, other] {
            let parents = post(Some(target.clone()), Some(target)).parents();
            assert_eq!(parents, PostParents::default());
        }
    }
}
