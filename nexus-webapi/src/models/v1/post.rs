use std::borrow::Cow;

use nexus_common::models::file::FileDetails;
use nexus_common::models::post::{self, Bookmark, PostCounts, PostRelationships};
use nexus_common::models::tag::TagDetails;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

use crate::models::{PostStreamDetailed, PostViewDetailed};

/// A post's kind under its `social/v1` name: `note`, `article`, the rest by name and `unknown`
/// for one the spec reader didn't recognise. The map is [`post::PostKind::v1_name`], the one
/// place the names are spelled; this type serializes through it and parses against it.
///
/// Reading is strict, unlike the stored kind's serde: a name that is not a `/v1` name,
/// `short`, `foo` or `Note`, is an error. It reads the `kind=` filter of the post streams
/// and the content search, so the error names that parameter; [`PostKinds`] reads
/// `exclude_kinds=` through [`PostKind::parse`] and names its own.
///
/// [`PostKinds`]: super::PostKinds
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostKind(pub post::PostKind);

/// The `/v1` names a filter takes, as the parameter descriptions list them.
pub const KIND_NAMES: &str = "note, article, image, video, link, file, collection, unknown";

impl PostKind {
    /// The `/v1` name of this kind.
    pub fn as_str(self) -> &'static str {
        self.0.v1_name()
    }

    /// The kind named `name` the `/v1` way, `unknown` included; any other name, a stored one
    /// or a differently cased one, is an error.
    pub fn parse(name: &str) -> Result<Self, String> {
        STORED_KINDS
            .into_iter()
            .find(|kind| kind.v1_name() == name)
            .map(Self)
            .ok_or_else(|| format!("`{name}` is not a post kind; the kinds are {KIND_NAMES}"))
    }
}

impl<'de> Deserialize<'de> for PostKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::parse(&name).map_err(|error| de::Error::custom(format!("`kind`: {error}")))
    }
}

impl From<post::PostKind> for PostKind {
    fn from(kind: post::PostKind) -> Self {
        Self(kind)
    }
}

/// The stored kind a parsed filter names, for the graph query and the search index.
impl From<PostKind> for post::PostKind {
    fn from(kind: PostKind) -> Self {
        kind.0
    }
}

impl Serialize for PostKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Every stored kind, so the schema can list each one's `/v1` name. A new variant has to be
/// added here as well; `tests` compares this list with the one serde derives for the stored
/// kind, so leaving it out fails a test rather than the document.
const STORED_KINDS: [post::PostKind; 8] = [
    post::PostKind::Short,
    post::PostKind::Long,
    post::PostKind::Image,
    post::PostKind::Video,
    post::PostKind::Link,
    post::PostKind::File,
    post::PostKind::Collection,
    post::PostKind::Unknown,
];

impl PartialSchema for PostKind {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::String)
            .enum_values(Some(STORED_KINDS.map(post::PostKind::v1_name)))
            .into()
    }
}

/// Named as the `/v0` component is: a client reads one document, and the kind's name is the
/// same concept in both, with the version's values.
impl ToSchema for PostKind {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("PostKind")
    }
}

/// A post's own fields, as `/v0`'s [`post::PostDetails`] with the kind under its `/v1` name.
#[derive(Serialize, ToSchema, Debug)]
pub struct PostDetails {
    pub content: String,
    pub id: String,
    pub indexed_at: i64,
    pub author: String,
    pub kind: PostKind,
    pub uri: String,
    /// The attachments the author named, in order.
    #[schema(required = true)]
    pub attachments: Vec<String>,
    /// `pubky://` URL of the lock server; `None` when the post is unlocked.
    pub lock: Option<String>,
    /// Set on the cleared node left behind when a post with relationships is deleted.
    pub deleted: bool,
}

impl From<post::PostDetails> for PostDetails {
    fn from(details: post::PostDetails) -> Self {
        Self {
            content: details.content,
            id: details.id,
            indexed_at: details.indexed_at,
            author: details.author,
            kind: details.kind.into(),
            uri: details.uri,
            attachments: details.attachments,
            lock: details.lock,
            deleted: details.deleted,
        }
    }
}

/// A post with its counts, tags and relationships, as `/v0`'s [`PostViewDetailed`] with the
/// kind under its `/v1` name.
#[derive(Serialize, ToSchema, Debug)]
pub struct PostView {
    pub details: PostDetails,
    pub counts: PostCounts,
    pub tags: Vec<TagDetails>,
    pub relationships: PostRelationships,
    pub bookmark: Option<Bookmark>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments_metadata: Vec<FileDetails>,
}

impl From<PostViewDetailed> for PostView {
    fn from(detailed: PostViewDetailed) -> Self {
        let PostViewDetailed {
            view,
            attachments_metadata,
        } = detailed;
        Self {
            details: view.details.into(),
            counts: view.counts,
            tags: view.tags,
            relationships: view.relationships,
            bookmark: view.bookmark,
            attachments_metadata,
        }
    }
}

/// A list of [`PostView`]s, as `/v0`'s [`PostStreamDetailed`].
#[derive(Serialize, ToSchema, Debug, Default)]
pub struct PostStream(pub Vec<PostView>);

impl From<PostStreamDetailed> for PostStream {
    fn from(stream: PostStreamDetailed) -> Self {
        Self(stream.0.into_iter().map(PostView::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// The `enum` values of a kind's schema.
    fn enum_values<T: PartialSchema>() -> Vec<Value> {
        let schema = serde_json::to_value(T::schema()).unwrap();
        schema["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("no enum in {schema}"))
            .clone()
    }

    /// Every stored kind serializes under its `/v1` name, and the schema lists that name.
    #[test]
    fn kinds_serialize_under_their_v1_name() {
        let listed = enum_values::<PostKind>();
        for kind in STORED_KINDS {
            let name = kind.v1_name();
            assert_eq!(serde_json::to_value(PostKind(kind)).unwrap(), name);
            assert!(
                listed.contains(&Value::from(name)),
                "{name} not in {listed:?}"
            );
        }
        assert_eq!(
            serde_json::to_value(PostKind(post::PostKind::Short)).unwrap(),
            "note"
        );
        assert_eq!(
            serde_json::to_value(PostKind(post::PostKind::Long)).unwrap(),
            "article"
        );
        assert_eq!(
            serde_json::to_value(PostKind(post::PostKind::Unknown)).unwrap(),
            "unknown"
        );
    }

    /// Every `/v1` name parses to its kind, `unknown` included, and round-trips.
    #[test]
    fn every_v1_name_parses_to_its_kind() {
        for kind in STORED_KINDS {
            let parsed = PostKind::parse(kind.v1_name()).unwrap();
            assert_eq!(parsed, PostKind(kind));
            assert_eq!(parsed.as_str(), kind.v1_name());
        }
        assert_eq!(PostKind::parse("note").unwrap().0, post::PostKind::Short);
        assert_eq!(PostKind::parse("article").unwrap().0, post::PostKind::Long);
        assert_eq!(
            PostKind::parse("unknown").unwrap().0,
            post::PostKind::Unknown
        );
    }

    /// A stored name, an unknown token, a differently cased name, whitespace and an empty
    /// string are all rejected, with the names a client may use.
    #[test]
    fn other_names_are_rejected() {
        for name in [
            "short", "long", "foo", "Note", "ARTICLE", " note", "note ", "",
        ] {
            let error = PostKind::parse(name).expect_err(name);
            assert!(error.contains(&format!("`{name}`")), "{error}");
            assert!(error.contains(KIND_NAMES), "{error}");
        }
    }

    /// `KIND_NAMES` lists every kind's `/v1` name, in the schema's order.
    #[test]
    fn kind_names_list_every_v1_name() {
        let listed: Vec<&str> = KIND_NAMES.split(", ").collect();
        assert_eq!(listed, STORED_KINDS.map(post::PostKind::v1_name));
    }

    /// Through serde, as a query string reads it: the error names the `kind` parameter.
    #[test]
    fn deserialize_is_strict_and_names_the_parameter() {
        #[derive(Deserialize)]
        struct Query {
            kind: Option<PostKind>,
        }
        let query: Query = serde_urlencoded::from_str("kind=note").unwrap();
        assert_eq!(query.kind, Some(PostKind(post::PostKind::Short)));
        let query: Query = serde_urlencoded::from_str("kind=unknown").unwrap();
        assert_eq!(query.kind, Some(PostKind(post::PostKind::Unknown)));
        let query: Query = serde_urlencoded::from_str("").unwrap();
        assert_eq!(query.kind, None);

        for name in ["short", "foo", "Note"] {
            let error = serde_urlencoded::from_str::<Query>(&format!("kind={name}"))
                .map(|query| query.kind)
                .expect_err(name)
                .to_string();
            assert!(error.starts_with("`kind`: "), "{error}");
            assert!(error.contains(&format!("`{name}`")), "{error}");
        }
    }

    /// `STORED_KINDS` names every variant serde knows: the derive on the stored kind is
    /// exhaustive, so a variant added there and not here shows up as a name this list lacks.
    #[test]
    fn stored_kinds_list_is_complete() {
        let derived = enum_values::<post::PostKind>();
        let listed: Vec<Value> = STORED_KINDS
            .iter()
            .map(|kind| Value::from(kind.as_str()))
            .collect();
        assert_eq!(listed, derived);
    }

    /// The `/v1` details are the stored details with the kind renamed and nothing else.
    #[test]
    fn details_differ_from_v0_only_in_the_kind_name() {
        let stored = post::PostDetails {
            content: "hi".into(),
            id: "post1".into(),
            indexed_at: 1,
            author: "author1".into(),
            kind: post::PostKind::Long,
            uri: "pubky://author1/pub/social/v1/posts/post1".into(),
            attachments: vec!["pubky://author1/pub/social/v1/files/f1".into()],
            lock: None,
            deleted: false,
        };
        let mut v0 = serde_json::to_value(&stored).unwrap();
        let v1 = serde_json::to_value(PostDetails::from(stored)).unwrap();
        assert_eq!(v1["kind"], "article");
        assert_eq!(v0["kind"], "long");
        v0["kind"] = json!("article");
        assert_eq!(v1, v0);
    }

    /// An empty stream and a post without attachment metadata keep `/v0`'s shape: a plain
    /// array, and no `attachments_metadata` key.
    #[test]
    fn stream_and_view_keep_the_v0_shape() {
        let stream = PostStream::from(PostStreamDetailed::default());
        assert_eq!(serde_json::to_value(stream).unwrap(), json!([]));

        let view = PostView::from(PostViewDetailed::new(
            nexus_common::models::post::PostView::default(),
            vec![],
        ));
        let json = serde_json::to_value(view).unwrap();
        assert!(json.get("attachments_metadata").is_none(), "{json}");
        assert_eq!(json["details"]["kind"], "note");
        assert_eq!(json["details"]["attachments"], json!([]));
    }
}
