//! The handlers' inputs: one plain struct per resource, holding exactly the fetched data its
//! handler reads, the address the object is stored under, and a [`Reference`] for each address
//! the object holds. The [`ResourceKey`] the event's path names travels beside the input.
//!
//! A translator fills them from a validated spec object, so the handlers never see the spec's
//! object layout, and translation alone decides each object's stored address. The inputs name no
//! spec type; the stored models they build still do, so each type crosses back beside the method
//! that builds its model.

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
    /// A tag: its tagger and tag id, the same whatever folder holds the tag file.
    Tag { key: ResourceKey },
    /// A resource outside Nexus's namespaces: an address of another scheme (`https`, `nostr`,
    /// …) or a public path under another app's namespace that is not a tag file.
    Foreign,
    /// Anything else inside Nexus's namespaces or no resource at all: a resource Nexus never
    /// references (a follow, a mute, a feed, a bookmark, a blob, `last_read`), a private path,
    /// a malformed `pubky.app` or `social` path, a `social/vN` epoch the pinned spec crate
    /// doesn't speak, a `pubky://` address with no namespace, or a string that isn't a URL.
    Other,
}

impl ReferenceTarget {
    /// The user who owns what the reference names; `None` for [`ReferenceTarget::Foreign`]
    /// and [`ReferenceTarget::Other`].
    pub fn owner_id(&self) -> Option<&PubkyId> {
        match self {
            Self::Post(key) | Self::File(key) | Self::Tag { key } => Some(&key.owner_id),
            Self::User(user_id) => Some(user_id),
            Self::Foreign | Self::Other => None,
        }
    }
}

/// What kind of post a [`PostInput`] is, in Nexus vocabulary: every epoch's translator maps its
/// spec's kinds into it.
///
/// It never leaves the watcher's memory, so it has no wire names. It moves to nexus-common with
/// the API work, which puts Nexus-owned types on the stored models.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostKind {
    Short,
    Long,
    Image,
    Video,
    Link,
    File,
    Collection,
    /// A kind the spec reader didn't recognise.
    Unknown,
}

/// A post, as the post handler reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct PostInput {
    /// The address the post is stored under.
    pub uri: String,
    pub content: String,
    pub kind: PostKind,
    /// If a reply, the parent the author named.
    pub parent: Option<Reference>,
    /// The embedded object the author named.
    pub embed: Option<Reference>,
    /// A collection post's items, in order; empty for any other kind. They feed author
    /// ingestion and, through [`PostInput::collected_posts`], the collected edges.
    pub collection_items: Vec<Reference>,
    /// The attachments the author named, in order; empty when the post has none.
    pub attachments: Vec<String>,
    /// `pubky://` URL of the lock server; `None` when the post is unlocked.
    pub lock: Option<String>,
}

impl PostInput {
    /// The details stored for the post. The attachments are stored as a list, even an empty
    /// one, never `null`.
    pub fn into_details(self, key: &ResourceKey) -> PostDetails {
        PostDetails {
            uri: self.uri,
            content: self.content,
            id: key.resource_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
            author: key.owner_id.to_string(),
            kind: self.kind.into(),
            attachments: Some(self.attachments),
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

    /// The posts the collection items name, in curator order, as `(author_id, post_id)`: the
    /// posts the collected edges point at. An item that names anything else (a profile, a file,
    /// a URL, an address that didn't resolve) gives no edge.
    pub fn collected_posts(&self) -> Vec<(PubkyId, String)> {
        self.collection_items
            .iter()
            .filter_map(|item| match &item.target {
                ReferenceTarget::Post(key) => Some((key.owner_id.clone(), key.resource_id.clone())),
                _ => None,
            })
            .collect()
    }
}

/// The stored post details still hold the v0 kind; [`PostInput::into_details`] is the one place
/// the kind crosses back.
impl From<PostKind> for PubkyAppPostKind {
    fn from(kind: PostKind) -> Self {
        match kind {
            PostKind::Short => Self::Short,
            PostKind::Long => Self::Long,
            PostKind::Image => Self::Image,
            PostKind::Video => Self::Video,
            PostKind::Link => Self::Link,
            PostKind::File => Self::File,
            PostKind::Collection => Self::Collection,
            PostKind::Unknown => Self::Unknown,
        }
    }
}

/// One link on a profile, in Nexus vocabulary: every epoch's translator maps its spec's links
/// into it.
///
/// The spec reader has validated and sanitised it before translation. It moves to nexus-common
/// with the API work, which puts Nexus-owned types on the stored models.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserLink {
    pub title: String,
    pub url: String,
}

/// A profile, as the user handler reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct UserInput {
    /// The address the profile is stored under.
    pub uri: String,
    pub name: String,
    pub bio: Option<String>,
    pub image: Option<String>,
    /// The profile's links, in order; empty when it has none.
    pub links: Vec<UserLink>,
    pub status: Option<String>,
}

impl UserInput {
    /// The details cached for the profile; [`UserInput::uri`] goes to the graph only. The links
    /// are stored as a list, even an empty one, never `null`.
    pub fn into_details(self, user_id: &PubkyId) -> UserDetails {
        UserDetails {
            name: self.name,
            bio: self.bio,
            status: self.status,
            links: Some(self.links.into_iter().map(Into::into).collect()),
            image: self.image,
            id: user_id.clone(),
            indexed_at: Utc::now().timestamp_millis(),
            deleted: false,
            epoch: Epoch::V0,
        }
    }
}

/// The stored user details still hold v0 links; [`UserInput::into_details`] is the one place the
/// links cross back.
impl From<UserLink> for PubkyAppUserLink {
    fn from(UserLink { title, url }: UserLink) -> Self {
        Self { title, url }
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
            kind: PostKind::Short,
            parent,
            embed,
            collection_items: Vec::new(),
            attachments: Vec::new(),
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
    fn collected_posts_keeps_post_items_in_order() {
        const OTHER_POST_ID: &str = "003286NSMY491";
        let other_key = ResourceKey {
            resource_id: OTHER_POST_ID.into(),
            ..key()
        };
        let mut collection = post(None, None);
        collection.kind = PostKind::Collection;
        collection.collection_items = vec![
            reference(
                &format!("pubky://{AUTHOR}/pub/pubky.app/posts/{OTHER_POST_ID}"),
                ReferenceTarget::Post(other_key.clone()),
            ),
            reference(
                &format!("pubky://{AUTHOR}/pub/pubky.app/profile.json"),
                ReferenceTarget::User(key().owner_id),
            ),
            reference("https://example.com", ReferenceTarget::Foreign),
            reference(
                &format!("pubky://{AUTHOR}/pub/pubky.app/follows/{AUTHOR}"),
                ReferenceTarget::Other,
            ),
            reference(
                &format!("pubky://{AUTHOR}/pub/pubky.app/posts/{POST_ID}"),
                ReferenceTarget::Post(key()),
            ),
        ];
        assert_eq!(
            collection.collected_posts(),
            vec![
                (other_key.owner_id, OTHER_POST_ID.to_string()),
                (key().owner_id, POST_ID.to_string()),
            ],
            "post items in curator order, every other item dropped"
        );
    }

    #[test]
    fn post_without_items_collects_nothing() {
        assert!(post(None, None).collected_posts().is_empty());
    }

    #[test]
    fn non_post_parent_and_embed_give_no_edges() {
        // A file, a foreign resource and an unresolved address: none is a reply or repost edge.
        let file = reference(
            "pubky://x/pub/pubky.app/files/1",
            ReferenceTarget::File(key()),
        );
        let foreign = reference("https://example.com", ReferenceTarget::Foreign);
        let other = reference("pubky://x/pub/pubky.app/follows/y", ReferenceTarget::Other);
        for target in [file, foreign, other] {
            let parents = post(Some(target.clone()), Some(target)).parents();
            assert_eq!(parents, PostParents::default());
        }
    }
}
