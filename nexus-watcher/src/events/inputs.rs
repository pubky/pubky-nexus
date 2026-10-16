//! The handlers' inputs: one plain struct per resource, holding exactly the fetched data its
//! handler reads, and the [`ResourceKey`] the event's path names.
//!
//! A translator fills them from a validated spec object, so the handlers never see the spec's
//! object layout. A few field types (`PubkyAppPostKind`, `PubkyAppUserLink`) still come from the
//! spec, as they do in the stored models.

use chrono::Utc;
use nexus_common::models::file::{FileDetails, FileUrls};
use nexus_common::models::post::{PostDetails, PostRelationships};
use nexus_common::models::user::{Epoch, UserDetails};
use pubky_social_specs::legacy_v0::{ParsedUri, PubkyAppPostKind, PubkyAppUserLink, Resource};
use pubky_social_specs::PubkyId;

/// Which resource an event names: the user whose storage holds it and its id there, however the
/// epoch spells the address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceKey {
    pub owner_id: PubkyId,
    pub resource_id: String,
}

/// A post, as the post handler reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct PostInput {
    pub content: String,
    pub kind: PubkyAppPostKind,
    /// If a reply, the URI of the parent post, as the author wrote it.
    pub parent: Option<String>,
    /// The URI of the embedded object, as the author wrote it.
    pub embed_uri: Option<String>,
    pub attachments: Option<Vec<String>>,
    /// `pubky://` URL of the lock server; `None` when the post is unlocked.
    pub lock: Option<String>,
}

impl PostInput {
    /// `uri` is the address the post was read from: the event path.
    pub fn into_details(self, uri: String, key: &ResourceKey) -> PostDetails {
        PostDetails {
            uri,
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

    /// The reply and repost relationships the post's parent and embed URIs name.
    pub fn relationships(&self) -> PostRelationships {
        let mut relationship = PostRelationships::default();

        if let Some(parent_uri) = &self.parent {
            relationship.replied = ParsedUri::try_from(parent_uri.as_str()).ok()
        }

        // Only a post can be reposted; other embed targets stay plain embeds.
        if let Some(embed_uri) = &self.embed_uri {
            relationship.reposted = ParsedUri::try_from(embed_uri.as_str())
                .ok()
                .filter(|uri| matches!(uri.resource, Resource::Post(_)));
        }
        relationship
    }
}

/// A profile, as the user handler reads it.
///
/// No `PartialEq`: the v0 `PubkyAppUserLink` it carries has none.
#[derive(Clone, Debug)]
pub struct UserInput {
    pub name: String,
    pub bio: Option<String>,
    pub image: Option<String>,
    /// `None` when the profile has no links or `null`; an empty list stays `Some`, so the stored
    /// `null` and `[]` stay distinct.
    pub links: Option<Vec<PubkyAppUserLink>>,
    pub status: Option<String>,
}

impl UserInput {
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
    /// The URI of the tagged object, as the author wrote it.
    pub uri: String,
    pub label: String,
}

/// A bookmark, as the bookmark handler reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkInput {
    /// The URI of the bookmarked post, as the author wrote it.
    pub uri: String,
}

/// A file's metadata, as the file handler reads it. The handler downloads the blob from `src`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInput {
    pub name: String,
    /// The URI of the blob, as the author wrote it.
    pub src: String,
    pub content_type: String,
    pub size: usize,
    pub created_at: i64,
}

impl FileInput {
    /// `uri` is the address the file was read from: the event path.
    pub fn into_details(self, uri: String, key: &ResourceKey) -> FileDetails {
        let user_id = key.owner_id.to_string();
        let file_id = key.resource_id.clone();
        FileDetails {
            urls: FileUrls::new(&user_id, &file_id, &self.content_type),
            name: self.name,
            src: self.src,
            content_type: self.content_type,
            uri,
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
    use pubky_social_specs::legacy_v0::{file_uri_builder, post_uri_builder};

    const AUTHOR: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    fn post_embedding(uri: String) -> PostInput {
        PostInput {
            content: "x".into(),
            kind: PubkyAppPostKind::Short,
            parent: None,
            embed_uri: Some(uri),
            attachments: None,
            lock: None,
        }
    }

    #[test]
    fn post_embed_becomes_a_repost() {
        let uri = post_uri_builder(AUTHOR.into(), "003286NSMY490".into());
        let rel = post_embedding(uri).relationships();
        assert!(
            matches!(rel.reposted.as_ref(), Some(u) if matches!(u.resource, Resource::Post(_))),
            "a post embed should become a reposted relationship"
        );
    }

    #[test]
    fn non_post_embed_is_not_a_repost() {
        // A non-post embed (file URI) must not become a repost.
        let uri = file_uri_builder(AUTHOR.into(), "003286NSMY490".into());
        let rel = post_embedding(uri).relationships();
        assert!(
            rel.reposted.is_none(),
            "a non-post embed must not become a repost"
        );
    }
}
