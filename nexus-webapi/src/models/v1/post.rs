use std::borrow::Cow;
use std::sync::LazyLock;

use nexus_common::models::post::{self, PostCounts, PostRelationships};
use serde::de::{self, Deserializer, IgnoredAny};
use serde::{Deserialize, Serialize, Serializer};
use utoipa::openapi::path::{Parameter, ParameterBuilder, ParameterIn};
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::Required;
use utoipa::openapi::{RefOr, Schema};
use utoipa::{IntoParams, PartialSchema, ToSchema};

use super::TagDetails;
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
pub const KIND_NAMES: &str =
    "note, article, image, video, link, file, collection, event, rsvp, rating, availability, unknown";

impl PostKind {
    /// The `/v1` name of this kind.
    pub fn as_str(self) -> &'static str {
        self.0.v1_name()
    }

    /// The kind named `name` the `/v1` way, `unknown` included; any other name, a stored one
    /// or a differently cased one, is an error.
    pub fn parse(name: &str) -> Result<Self, String> {
        STORED_KINDS
            .iter()
            .copied()
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

/// Every stored kind, in the stored schema's order, so the schema can list each one's `/v1`
/// name and the parser can find each one. Read from the names serde derives for the stored
/// kind, its schema's `enum` list, each read back as the kind it names: a new variant is in
/// the derive, and so here, with no list to keep in step; [`post::PostKind::v1_name`] is
/// exhaustive, so the compiler asks for its `/v1` name.
static STORED_KINDS: LazyLock<Vec<post::PostKind>> = LazyLock::new(|| {
    let schema = serde_json::to_value(post::PostKind::schema()).expect("a schema serializes");
    schema["enum"]
        .as_array()
        .unwrap_or_else(|| panic!("the stored kind's schema lists no names: {schema}"))
        .iter()
        .map(|name| {
            serde_json::from_value(name.clone())
                .unwrap_or_else(|error| panic!("{name} is not a stored kind: {error}"))
        })
        .collect()
});

impl PartialSchema for PostKind {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::String)
            .enum_values(Some(STORED_KINDS.iter().map(|kind| kind.v1_name())))
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
/// kind under its `/v1` name, without the viewer's `bookmark`, without
/// `attachments_metadata` and with the tags without `relationship`. `social/v1` bookmarks
/// live under `/priv/`, which Nexus never sees; a `social/v1` file is bytes behind
/// `/v1/static` with no `File` node, so a `/v1` view never promises metadata that does not
/// exist for it; and a tag's `tag_uri` says whether the viewer tagged it.
#[derive(Serialize, ToSchema, Debug)]
pub struct PostView {
    /// The average of the ratings the post received, 1–10; 0 with none. `/v0`, frozen, does
    /// not serve it. Plain fields, not the flattened summary: the view's schema stays an object
    /// with its own properties.
    pub rating_avg: f64,
    /// How many ratings the post received.
    pub rating_count: i64,
    pub details: PostDetails,
    pub counts: PostCounts,
    pub tags: Vec<TagDetails>,
    pub relationships: PostRelationships,
}

/// The detailed view's metadata is dropped, never fetched: `/v1` reads
/// [`NoAttachmentMetadata`], so the body fetches none.
impl From<PostViewDetailed> for PostView {
    fn from(detailed: PostViewDetailed) -> Self {
        let PostViewDetailed {
            view,
            attachments_metadata: _,
        } = detailed;
        let post::PostView {
            rating,
            details,
            counts,
            tags,
            relationships,
            bookmark: _,
        } = view;
        Self {
            rating_avg: rating.rating_avg,
            rating_count: rating.rating_count,
            details: details.into(),
            counts,
            tags: tags.into_iter().map(TagDetails::from).collect(),
            relationships,
        }
    }
}

/// What `/v1` reads where `/v0` reads `include_attachment_metadata`: nothing. The post view,
/// the post stream and its `by_ids` body take the flag through the version, so a `/v1` query
/// has no such parameter and documents none. A client that sends it anyway is read as any
/// unknown parameter is, whatever the value: ignored, with no 400 of its own.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq)]
#[serde(transparent)]
pub struct NoAttachmentMetadata(IgnoredAny);

/// Never fetch: there is no `File` node behind a `social/v1` file.
impl From<NoAttachmentMetadata> for bool {
    fn from(_: NoAttachmentMetadata) -> Self {
        false
    }
}

/// No parameter to document.
impl IntoParams for NoAttachmentMetadata {
    fn into_params(_: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        Vec::new()
    }
}

/// `item_kind=` on the post streams, with `kind=collection`: the kind of post a listed
/// Collection must hold, in the `/v1` names. `/v1` only: `/v0` is frozen and reads nothing
/// under that name ([`NoItemKind`](crate::models::v0_frozen::NoItemKind)).
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq)]
#[serde(transparent)]
pub struct ItemKind(pub Option<PostKind>);

/// The stored kind a listed post must have, for the graph query.
impl From<ItemKind> for Option<post::PostKind> {
    fn from(kind: ItemKind) -> Self {
        kind.0.map(Into::into)
    }
}

/// What `item_kind=` does, as the parameter description says it.
pub const ITEM_KIND_DESCRIPTION: &str = "Requires `kind=collection`. Returns only Collection posts that list at least one live indexed post of this kind, e.g. `kind=collection&item_kind=event` for collections containing events (calendars). Each collection's upcoming events are served by `GET /v1/stream/events?collection=<collection URI>`. External (non-post) items do not count. Any other `kind` (or none) returns 400, and so does `unknown`.";

/// The parameter, after the route's other query parameters, with the `/v1` kind names.
impl IntoParams for ItemKind {
    fn into_params(_: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        vec![ParameterBuilder::new()
            .name("item_kind")
            .parameter_in(ParameterIn::Query)
            .description(Some(ITEM_KIND_DESCRIPTION))
            .required(Required::False)
            .schema(Some(PostKind::schema()))
            .build()]
    }
}

/// A stored view with no attachment metadata fetched, as the bootstrap lists its posts.
impl From<post::PostView> for PostView {
    fn from(view: post::PostView) -> Self {
        PostViewDetailed::new(view, Vec::new()).into()
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

/// The stored stream the bootstrap lists, with no attachment metadata on any post.
impl From<post::PostStream> for PostStream {
    fn from(stream: post::PostStream) -> Self {
        Self(stream.0.into_iter().map(PostView::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_helpers::enum_values;
    use serde_json::json;

    /// Every stored kind serializes under its `/v1` name, and the schema lists that name.
    #[test]
    fn kinds_serialize_under_their_v1_name() {
        let listed = enum_values::<PostKind>();
        for kind in STORED_KINDS.iter().copied() {
            let name = kind.v1_name();
            assert_eq!(serde_json::to_value(PostKind(kind)).unwrap(), name);
            assert!(
                listed.iter().any(|listed| listed == name),
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
        for kind in STORED_KINDS.iter().copied() {
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
        let stored: Vec<&str> = STORED_KINDS.iter().map(|kind| kind.v1_name()).collect();
        assert_eq!(listed, stored);
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

    /// `STORED_KINDS` is every variant serde knows, each read back as itself: the list is the
    /// derive's, in its order, `Unknown` included.
    #[test]
    fn stored_kinds_are_the_derived_names() {
        let derived = enum_values::<post::PostKind>();
        let listed: Vec<String> = STORED_KINDS
            .iter()
            .map(|kind| kind.as_str().to_string())
            .collect();
        assert_eq!(listed, derived);
        assert!(STORED_KINDS.contains(&post::PostKind::Unknown));
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

    /// An empty stream and a post without attachment metadata keep `/v0`'s shape, a plain
    /// array and no `attachments_metadata` key, and the view has no `bookmark` key whether or
    /// not the viewer bookmarked the post.
    #[test]
    fn stream_and_view_keep_the_v0_shape_without_bookmark() {
        let stream = PostStream::from(PostStreamDetailed::default());
        assert_eq!(serde_json::to_value(stream).unwrap(), json!([]));

        let bookmarked = post::PostView {
            bookmark: Some(post::Bookmark {
                id: "bookmark1".into(),
                indexed_at: 1,
            }),
            ..Default::default()
        };
        for view in [post::PostView::default(), bookmarked] {
            let v0 = serde_json::to_value(&view).unwrap();
            assert!(v0.get("bookmark").is_some(), "{v0}");
            let json =
                serde_json::to_value(PostView::from(PostViewDetailed::new(view, vec![]))).unwrap();
            assert!(json.get("bookmark").is_none(), "{json}");
            assert!(json.get("attachments_metadata").is_none(), "{json}");
            assert_eq!(json["details"]["kind"], "note");
            assert_eq!(json["details"]["attachments"], json!([]));
        }
    }

    /// A detailed view that did fetch metadata converts without it: `/v0` writes
    /// `attachments_metadata`, the `/v1` view has no such key and no such property, and is
    /// otherwise the `/v0` view.
    #[test]
    fn view_drops_fetched_attachment_metadata() {
        let file = nexus_common::models::file::FileDetails {
            id: "2ZK3A1B2C3D40".into(),
            owner_id: "owner".into(),
            name: "file1".into(),
            content_type: "image/png".into(),
            ..Default::default()
        };
        let detailed = PostViewDetailed::new(post::PostView::default(), vec![file]);
        let mut v0 = serde_json::to_value(&detailed).unwrap();
        assert_eq!(
            v0["attachments_metadata"].as_array().map(Vec::len),
            Some(1),
            "{v0}"
        );
        let v1 = serde_json::to_value(PostView::from(detailed)).unwrap();
        assert!(v1.get("attachments_metadata").is_none(), "{v1}");
        for field in ["details", "counts", "tags", "relationships"] {
            assert!(v1.get(field).is_some(), "{field}: {v1}");
        }
        v0.as_object_mut().unwrap().remove("attachments_metadata");
        v0.as_object_mut().unwrap().remove("bookmark");
        v0["details"]["kind"] = json!("note");
        // The rating summary is `/v1`'s alone.
        v0["rating_avg"] = json!(0.0);
        v0["rating_count"] = json!(0);
        assert_eq!(v1, v0);

        let schema = serde_json::to_value(PostView::schema()).unwrap();
        let properties = schema["properties"].as_object().unwrap();
        assert!(!properties.contains_key("attachments_metadata"), "{schema}");
        assert!(!properties.contains_key("bookmark"), "{schema}");
        // The rating summary is two of the view's own properties: an object schema, no `allOf`.
        assert!(schema.get("allOf").is_none(), "{schema}");
        for field in ["rating_avg", "rating_count"] {
            assert!(properties.contains_key(field), "{field}: {schema}");
        }
    }

    /// `NoAttachmentMetadata` reads whatever a client sends under the flag's name and never
    /// asks for a fetch, where `/v0`'s flag reads `true` and `false` and rejects the rest; it
    /// documents no parameter, where `/v0`'s documents the one.
    #[test]
    fn no_attachment_metadata_ignores_the_flag_and_documents_nothing() {
        use crate::models::IncludeAttachmentMetadata;

        #[derive(Deserialize)]
        struct Query<F> {
            #[serde(default)]
            include_attachment_metadata: F,
        }
        for query in [
            "",
            "include_attachment_metadata=true",
            "include_attachment_metadata=false",
            "include_attachment_metadata=1",
            "include_attachment_metadata=maybe",
            "include_attachment_metadata=",
        ] {
            let parsed: Query<NoAttachmentMetadata> =
                serde_urlencoded::from_str(query).expect(query);
            assert!(!bool::from(parsed.include_attachment_metadata), "{query}");
        }
        let fetch = |query: &str| {
            serde_urlencoded::from_str::<Query<IncludeAttachmentMetadata>>(query)
                .map(|parsed| bool::from(parsed.include_attachment_metadata))
        };
        assert!(!fetch("").unwrap());
        assert!(fetch("include_attachment_metadata=true").unwrap());
        assert!(!fetch("include_attachment_metadata=false").unwrap());
        for query in [
            "include_attachment_metadata=1",
            "include_attachment_metadata=maybe",
        ] {
            assert!(fetch(query).is_err(), "{query}");
        }

        assert!(NoAttachmentMetadata::into_params(|| None).is_empty());
        let documented = IncludeAttachmentMetadata::into_params(|| None);
        assert_eq!(documented.len(), 1);
        assert_eq!(
            serde_json::to_value(&documented[0]).unwrap(),
            json!({
                "name": "include_attachment_metadata",
                "in": "query",
                "description": "Include file metadata for post attachments",
                "required": false,
                "schema": { "type": "boolean" }
            })
        );
    }

    /// A stored view and a stored stream, as the bootstrap lists them, convert like the
    /// detailed ones with no metadata: no `bookmark`, no `attachments_metadata`.
    #[test]
    fn stored_view_and_stream_convert_without_bookmark() {
        let bookmarked = post::PostView {
            bookmark: Some(post::Bookmark {
                id: "bookmark1".into(),
                indexed_at: 1,
            }),
            ..Default::default()
        };
        let from_stored = serde_json::to_value(PostView::from(bookmarked)).unwrap();
        let from_detailed = serde_json::to_value(PostView::from(PostViewDetailed::new(
            post::PostView::default(),
            vec![],
        )))
        .unwrap();
        assert_eq!(from_stored, from_detailed);
        assert!(from_stored.get("bookmark").is_none(), "{from_stored}");
        assert!(
            from_stored.get("attachments_metadata").is_none(),
            "{from_stored}"
        );

        let stream = PostStream::from(post::PostStream(vec![post::PostView::default()]));
        let json = serde_json::to_value(stream).unwrap();
        assert_eq!(json.as_array().map(Vec::len), Some(1), "{json}");
        assert_eq!(json[0], from_detailed);
    }
}
