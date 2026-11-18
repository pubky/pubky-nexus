//! The `/v0` post shapes: the stored models as `/v0` has always written them.
//!
//! `/v0` is frozen. A post's stored relationships also hold an external embed and a reply to
//! an external resource, which `/v1` serves; `/v0` has no `embedded`, and names in `replied`
//! only a post, `null` for a reply to anything else. Each shape is a plain struct converted
//! from the stored one, so serde writes the fields in their declared order, the stored
//! models' order, and the conversions destructure the stored post view, post stream,
//! relationships and bootstrap: a field added to one of them does not compile until it is
//! given to `/v0` or left out here. The models they embed (`PostDetails`, `PostCounts`,
//! `TagDetails`, `Bookmark`, `FileDetails`, `UserStream`, `Notification`) are served as stored. The responses that
//! embed a post view, the post stream and the bootstrap, are `/v0` types too; each is named
//! as the stored model is, so the `/v0` document keeps its schema names. The document also
//! keeps its descriptions, which utoipa reads from doc comments: the fields carry the ones
//! `/v0` has always documented, and the notes on each type are plain comments.

use nexus_common::models::bootstrap::{self, BootstrapIds};
use nexus_common::models::file::FileDetails;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::{self, Bookmark, PostCounts, PostDetails};
use nexus_common::models::tag::TagDetails;
use nexus_common::models::user::UserStream;
use pubky_social_specs::PubkyId;
use serde::Serialize;
use utoipa::ToSchema;

// A post's relationships on `/v0`: the post it replies to, the post it reposts and the
// users it mentions. No `embedded`, and no external `replied`.
#[derive(Serialize, ToSchema, Debug)]
pub struct PostRelationships {
    /// If set, URI of the post this is a reply to
    pub replied: Option<String>,

    /// If set, URI of the post this post is reposting
    pub reposted: Option<String>,

    /// List of user IDs mentioned in this post
    pub mentioned: Vec<PubkyId>,
}

/// `replied` is kept only when the graph read it as a post; the embed is dropped.
impl From<post::PostRelationships> for PostRelationships {
    fn from(relationships: post::PostRelationships) -> Self {
        let post::PostRelationships {
            replied,
            replied_external,
            reposted,
            embedded: _,
            mentioned,
        } = relationships;
        Self {
            replied: replied.filter(|_| !replied_external),
            reposted,
            mentioned,
        }
    }
}

// A post with its counts, tags, relationships and the viewer's bookmark. The schema keeps
// the description `/v0` has always documented.
#[derive(Serialize, ToSchema, Debug)]
#[schema(
    description = "Represents a Pubky user with relational data including tags, counts, and relationship with a viewer."
)]
pub struct PostView {
    pub details: PostDetails,
    pub counts: PostCounts,
    pub tags: Vec<TagDetails>,
    pub relationships: PostRelationships,
    pub bookmark: Option<Bookmark>,
}

impl From<post::PostView> for PostView {
    fn from(view: post::PostView) -> Self {
        let post::PostView {
            details,
            counts,
            tags,
            relationships,
            bookmark,
        } = view;
        Self {
            details,
            counts,
            tags,
            relationships: relationships.into(),
            bookmark,
        }
    }
}

// A post view with the metadata of its attachments, when asked for and found.
#[derive(Serialize, ToSchema, Debug)]
pub struct PostViewDetailed {
    #[serde(flatten)]
    pub view: PostView,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments_metadata: Vec<FileDetails>,
}

impl From<crate::models::PostViewDetailed> for PostViewDetailed {
    fn from(detailed: crate::models::PostViewDetailed) -> Self {
        let crate::models::PostViewDetailed {
            view,
            attachments_metadata,
        } = detailed;
        Self {
            view: view.into(),
            attachments_metadata,
        }
    }
}

// A list of post views, `GET /stream/posts` and `POST /stream/posts/by_ids`.
#[derive(Serialize, ToSchema, Debug)]
pub struct PostStreamDetailed(pub Vec<PostViewDetailed>);

impl From<crate::models::PostStreamDetailed> for PostStreamDetailed {
    fn from(stream: crate::models::PostStreamDetailed) -> Self {
        Self(stream.0.into_iter().map(PostViewDetailed::from).collect())
    }
}

// A list of post views without attachment metadata, as the bootstrap lists its posts.
#[derive(Serialize, ToSchema, Debug)]
pub struct PostStream(pub Vec<PostView>);

impl From<post::PostStream> for PostStream {
    fn from(stream: post::PostStream) -> Self {
        Self(stream.0.into_iter().map(PostView::from).collect())
    }
}

// Everything the client app loads at sign-in, with its posts in their `/v0` shape.
#[derive(Serialize, ToSchema, Debug)]
pub struct Bootstrap {
    /// The user objects shown to the given user ID
    pub users: UserStream,
    /// The posts objects shown to the given user ID
    pub posts: PostStream,
    /// File metadata for all attachments referenced in posts and replies
    pub files: Vec<FileDetails>,
    /// IDs of objects shown to this user on the home page of the FE
    pub ids: BootstrapIds,
    /// Whether or not this user is already indexed
    pub indexed: bool,
    /// Latest notifications
    pub notifications: Vec<Notification>,
}

impl From<bootstrap::Bootstrap> for Bootstrap {
    fn from(bootstrap: bootstrap::Bootstrap) -> Self {
        let bootstrap::Bootstrap {
            users,
            posts,
            files,
            ids,
            indexed,
            notifications,
        } = bootstrap;
        Self {
            users,
            posts: posts.into(),
            files,
            ids,
            indexed,
            notifications,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::models::notification::NotificationBody;
    use nexus_common::models::post::PostKind;
    use serde_json::json;
    use utoipa::PartialSchema;

    const AUTHOR: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

    /// A stored view of a post with no external reference, every field filled: a reply to a
    /// post that mentions a user, with a tag and the viewer's bookmark.
    fn stored_view(id: &str) -> post::PostView {
        serde_json::from_value(json!({
            "details": {
                "content": "a reply",
                "id": id,
                "indexed_at": 1_724_134_095_000_i64,
                "author": AUTHOR,
                "kind": "short",
                "uri": format!("pubky://{AUTHOR}/pub/pubky.app/posts/{id}"),
                "attachments": [format!("pubky://{AUTHOR}/pub/pubky.app/files/0033SSE3B1FQ0")],
                "lock": null,
                "deleted": false
            },
            "counts": {"tags": 1, "unique_tags": 1, "replies": 2, "reposts": 0, "collections": 0},
            "tags": [{
                "label": "news",
                "taggers": [AUTHOR],
                "taggers_count": 1,
                "relationship": true,
                "tag_uri": format!("pubky://{AUTHOR}/pub/pubky.app/tags/TAG1")
            }],
            "relationships": {
                "replied": format!("pubky://{AUTHOR}/pub/pubky.app/posts/00000039YD9BM"),
                "reposted": null,
                "mentioned": [AUTHOR]
            },
            "bookmark": {"id": "BOOKMARK1", "indexed_at": 1}
        }))
        .unwrap()
    }

    fn file() -> FileDetails {
        FileDetails {
            id: "0033SSE3B1FQ0".into(),
            owner_id: AUTHOR.into(),
            name: "file1".into(),
            content_type: "image/png".into(),
            ..Default::default()
        }
    }

    /// For a post with no external reference, the `/v0` view, stream and bootstrap are written
    /// byte for byte as the stored models write them: the same field names in the same order.
    /// The relationships list `replied`, `reposted` and `mentioned`, in that order.
    #[test]
    fn v0_shapes_write_the_stored_bytes_for_a_post_without_external_references() {
        let detailed =
            || crate::models::PostViewDetailed::new(stored_view("0033SSE3B1FQ1"), vec![file()]);
        let stored = serde_json::to_string(&detailed()).unwrap();
        let v0 = serde_json::to_string(&PostViewDetailed::from(detailed())).unwrap();
        assert_eq!(v0, stored);
        let relationships = format!(
            r#""relationships":{{"replied":"pubky://{AUTHOR}/pub/pubky.app/posts/00000039YD9BM","reposted":null,"mentioned":["{AUTHOR}"]}},"bookmark":{{"#
        );
        assert!(v0.contains(&relationships), "{v0}");
        assert!(
            v0.starts_with(r#"{"details":{"content":"a reply","#),
            "{v0}"
        );

        let stream = || {
            crate::models::PostStreamDetailed(vec![
                detailed(),
                crate::models::PostViewDetailed::new(stored_view("0033SSE3B1FQ2"), vec![]),
            ])
        };
        let stored = serde_json::to_string(&stream()).unwrap();
        let v0 = serde_json::to_string(&PostStreamDetailed::from(stream())).unwrap();
        assert_eq!(v0, stored);

        let bootstrap = || bootstrap::Bootstrap {
            posts: post::PostStream(vec![stored_view("0033SSE3B1FQ3")]),
            files: vec![file()],
            ids: BootstrapIds {
                stream: vec![format!("{AUTHOR}:0033SSE3B1FQ3")],
                ..Default::default()
            },
            indexed: true,
            notifications: vec![Notification {
                timestamp: 7,
                body: NotificationBody::Mention {
                    mentioned_by: AUTHOR.into(),
                    post_uri: "u".into(),
                    post_kind: PostKind::Long,
                },
            }],
            ..Default::default()
        };
        let stored = serde_json::to_string(&bootstrap()).unwrap();
        let v0 = serde_json::to_string(&Bootstrap::from(bootstrap())).unwrap();
        assert_eq!(v0, stored);
        assert!(
            v0.starts_with(r#"{"users":[],"posts":[{"details":"#),
            "{v0}"
        );
    }

    /// A reply to an external resource reads `null` and an external embed is left out, while
    /// the stored relationships, the cache's, keep both; a reply to a post keeps its address
    /// whatever its path, the flag the graph read deciding.
    #[test]
    fn external_references_stay_out_of_v0() {
        let external = post::PostRelationships {
            replied: Some("https://example.com/a".into()),
            replied_external: true,
            embedded: Some("https://example.com/b".into()),
            mentioned: vec![PubkyId::try_from(AUTHOR).unwrap()],
            ..Default::default()
        };
        let stored = serde_json::to_value(&external).unwrap();
        assert_eq!(stored["replied"], "https://example.com/a");
        assert_eq!(stored["embedded"], "https://example.com/b");
        assert_eq!(
            serde_json::to_string(&PostRelationships::from(external)).unwrap(),
            format!(r#"{{"replied":null,"reposted":null,"mentioned":["{AUTHOR}"]}}"#)
        );

        let post = format!("pubky://{AUTHOR}/pub/social/v1/posts/00000039YD9BM");
        let reply = post::PostRelationships {
            replied: Some(post.clone()),
            ..Default::default()
        };
        assert_eq!(PostRelationships::from(reply).replied, Some(post));
    }

    /// The stored relationships read a cached value written without the flag as a reply to a
    /// post, and write no flag for one: the cache keeps its shape for every reply to a post.
    #[test]
    fn stored_flag_defaults_to_a_post_and_is_written_only_when_set() {
        let post = format!("pubky://{AUTHOR}/pub/pubky.app/posts/00000039YD9BM");
        let cached = json!({"replied": post, "reposted": null, "mentioned": []});
        let read: post::PostRelationships = serde_json::from_value(cached.clone()).unwrap();
        assert!(!read.replied_external);
        assert_eq!(serde_json::to_value(&read).unwrap(), cached);
        assert_eq!(PostRelationships::from(read).replied, Some(post));

        let external = post::PostRelationships {
            replied: Some("https://example.com/a".into()),
            replied_external: true,
            ..Default::default()
        };
        let written = serde_json::to_value(&external).unwrap();
        assert_eq!(written["replied_external"], true, "{written}");
        let read: post::PostRelationships = serde_json::from_value(written).unwrap();
        assert!(read.replied_external);
    }

    /// Each shape is documented under the stored model's name, so the `/v0` document keeps
    /// its references; the relationships have no `embedded` and no flag.
    #[test]
    fn shapes_are_named_as_the_stored_models() {
        assert_eq!(PostView::name(), post::PostView::name());
        assert_eq!(PostStream::name(), post::PostStream::name());
        assert_eq!(PostRelationships::name(), post::PostRelationships::name());
        assert_eq!(
            PostViewDetailed::name(),
            crate::models::PostViewDetailed::name()
        );
        assert_eq!(
            PostStreamDetailed::name(),
            crate::models::PostStreamDetailed::name()
        );
        assert_eq!(Bootstrap::name(), bootstrap::Bootstrap::name());
        let schema = serde_json::to_value(PostRelationships::schema()).unwrap();
        let properties: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
        assert_eq!(properties, ["mentioned", "replied", "reposted"], "{schema}");
    }
}
