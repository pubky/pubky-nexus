//! `/v0` is frozen: what the stored models gained since stays out of it.
//!
//! External post references give a post's stored relationships an `embedded` URI, an external
//! embed, and let `replied` name an external resource. The cache keeps both, and `/v1` serves
//! them. `/v0` serves what it served before: no `embedded`, and a `replied` only when it names
//! a post, `null` otherwise, as its graph read answered for a reply to anything but a post.
//!
//! The stored post kind holds the event kinds (`event`, `rsvp`, `rating`, `availability`)
//! under their own names, the stored notification body has an `event_changed` variant, and
//! the stream sortings have `rating`. `/v1` serves all of them. `/v0` serves what it served
//! before Nexus read them: an event kind is a kind it does not know, `unknown`, as the
//! translator stored it before; the `event_changed` notification and the `rating` sorting do
//! not exist. Requests that name them are refused or read as `/v0` always read an unknown
//! name, and the routes that only serve them are `/v1`'s own.
//!
//! The responses `/v0` shares with `/v1` are written through [`V0Shape`], which serializes the
//! stored model and then takes these additions out ([`freeze`]); [`freeze_docs`] does the
//! same to the `/v0` document's shared schemas. `/v0`'s `kind=` reads through [`V0PostKind`].

use std::borrow::Cow;

use nexus_common::models::post::PostKind;
use serde::de::{Deserializer, IgnoredAny};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::openapi::path::{Parameter, ParameterIn};
use utoipa::openapi::{OpenApi, RefOr, Schema};
use utoipa::{IntoParams, PartialSchema, ToSchema};

/// The `type` tag of the notification body `/v0` does not have.
const EVENT_CHANGED: &str = "event_changed";
/// The stream sorting `/v0` does not have.
const RATING_SORTING: &str = "rating";

/// A `/v0` response: the stored model `T`, written in the frozen vocabulary ([`freeze`]).
/// Documented as `T` is, under `T`'s name, so the `/v0` document keeps its references.
#[derive(Debug)]
pub struct V0Shape<T>(pub T);

impl<T> From<T> for V0Shape<T> {
    fn from(model: T) -> Self {
        Self(model)
    }
}

impl<T: Serialize> Serialize for V0Shape<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut value = serde_json::to_value(&self.0).map_err(serde::ser::Error::custom)?;
        freeze(&mut value);
        value.serialize(serializer)
    }
}

impl<T: ToSchema> PartialSchema for V0Shape<T> {
    fn schema() -> RefOr<Schema> {
        T::schema()
    }
}

impl<T: ToSchema> ToSchema for V0Shape<T> {
    fn name() -> Cow<'static, str> {
        T::name()
    }
}

/// Takes what `/v0` does not have out of a serialized stored model, at any depth: a post kind
/// (`kind` of a post's details, `post_kind` of a notification) that is not a `/v0` kind becomes
/// `unknown`, an `event_changed` notification is dropped from a `notifications` list, and a
/// post's `relationships` lose `embedded` and read `null` for a `replied` that names no post
/// ([`freeze_relationships`]). Every other value, an unrecognised kind included, is left as it
/// is.
pub fn freeze(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                if key == "kind" || key == "post_kind" {
                    freeze_kind(field);
                    continue;
                }
                if key == "relationships" {
                    freeze_relationships(field);
                }
                if key == "notifications" {
                    if let Value::Array(items) = field {
                        items.retain(|item| !is_event_changed(item));
                    }
                }
                freeze(field);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(freeze),
        _ => {}
    }
}

/// A post's relationships as `/v0` has them: no `embedded`, and `replied` only for a post. A
/// value that is not a post's relationships, which always list `mentioned`, is left alone.
fn freeze_relationships(relationships: &mut Value) {
    let Value::Object(fields) = relationships else {
        return;
    };
    if !fields.contains_key("mentioned") {
        return;
    }
    fields.remove("embedded");
    let external = fields
        .get("replied")
        .and_then(Value::as_str)
        .is_some_and(|uri| !is_post_uri(uri));
    if external {
        fields.insert("replied".to_string(), Value::Null);
    }
}

/// Whether `uri` is a post's stored address: its path, right after the authority, starts in
/// either epoch's folder. Only a post's address does, so this tells a reply to a post from a
/// reply to an external resource (another scheme's URL or another app's object, even one whose
/// path nests a post folder further down) without parsing the id, and so without refusing a
/// post whose id an older writer spelled loosely.
fn is_post_uri(uri: &str) -> bool {
    uri.strip_prefix("pubky://")
        .and_then(|rest| rest.find('/').map(|slash| &rest[slash..]))
        .is_some_and(|path| POST_FOLDERS.iter().any(|folder| path.starts_with(folder)))
}

/// The folders a post's stored address names, `pubky.app`'s and `social/v1`'s.
const POST_FOLDERS: [&str; 2] = ["/pub/pubky.app/posts/", "/pub/social/v1/posts/"];

/// A stored kind name `/v0` does not know written as `unknown`.
fn freeze_kind(field: &mut Value) {
    let Some(name) = field.as_str() else {
        return;
    };
    if let Ok(kind) = serde_json::from_value::<PostKind>(Value::from(name)) {
        if !kind.is_v0() {
            *field = Value::from(PostKind::Unknown.as_str());
        }
    }
}

/// The stored kinds `/v0` reads as `unknown`: `unknown` itself and every kind it does not know.
/// `/v0`'s `kind=` filter for `unknown` takes them all, so it matches every post `/v0` shows as
/// `unknown`, as it did before the event kinds were stored under their own names. Read from
/// the stored kind's schema, so a kind added later is in it with no list to keep in step.
pub fn read_as_unknown() -> Vec<PostKind> {
    let schema = serde_json::to_value(PostKind::schema()).expect("a schema serializes");
    schema["enum"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|name| serde_json::from_value::<PostKind>(name.clone()).ok())
        .filter(|kind| *kind == PostKind::Unknown || !kind.is_v0())
        .collect()
}

/// [`read_as_unknown`] as the content search's tag filter takes alternatives, `a|b|c`.
pub const UNKNOWN_SEARCH_TAGS: &str = "unknown|event|rsvp|rating|availability";

/// Whether a serialized notification is an `event_changed` one.
pub fn is_event_changed(notification: &Value) -> bool {
    notification["body"]["type"] == EVENT_CHANGED
}

/// `kind=` as `/v0` reads it: a stored name, and any other name, an event kind's included, as
/// `unknown`, as `/v0` has always read a name it does not know. Documented as the stored kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct V0PostKind(pub PostKind);

impl<'de> Deserialize<'de> for V0PostKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let kind = PostKind::deserialize(deserializer)?;
        Ok(Self(if kind.is_v0() {
            kind
        } else {
            PostKind::Unknown
        }))
    }
}

impl From<V0PostKind> for PostKind {
    fn from(kind: V0PostKind) -> Self {
        kind.0
    }
}

impl PartialSchema for V0PostKind {
    fn schema() -> RefOr<Schema> {
        PostKind::schema()
    }
}

impl ToSchema for V0PostKind {
    fn name() -> Cow<'static, str> {
        PostKind::name()
    }
}

/// The scores of the key streams and the tag search as `/v0` documents them: whole numbers, as
/// they were before a fractional rating average could be one, with their `/v0` descriptions.
/// `/v0` still writes them as whole numbers: no `/v0` sorting has another score. Schema only,
/// never built.
#[derive(ToSchema)]
#[allow(dead_code)]
struct V0Scores {
    /// Latest tag timestamp of the last entry, usable as the `start` cursor for
    /// the next page. `None` when the page is empty.
    ///
    /// The cursor carries only the score, not the id, and `start` is
    /// inclusive, so a resume re-selects every row sharing the cursor score and
    /// takes a `skip` covering the served ones. Timestamps rarely tie, so
    /// `skip=1` resumes a timeline walk.
    last_score: Option<u64>,
    last_post_score: Option<u64>,
    score: usize,
}

/// What `/v0` reads where `/v1` reads `item_kind`: nothing. `/v0` had no such parameter, so it
/// reads one as any unknown parameter, whatever the value: ignored, with no 400 of its own.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq)]
#[serde(transparent)]
pub struct NoItemKind(IgnoredAny);

/// No kind to filter collections by.
impl From<NoItemKind> for Option<PostKind> {
    fn from(_: NoItemKind) -> Self {
        None
    }
}

/// No parameter to document.
impl IntoParams for NoItemKind {
    fn into_params(_: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        Vec::new()
    }
}

/// The `/v0` document in the frozen vocabulary: the shared schemas the stored models document
/// lose what was added since. `PostRelationships` has no `embedded`, `PostKind` lists the
/// `/v0` kinds and `unknown`, `NotificationType` and `NotificationBody` have no
/// `event_changed`, `StreamSorting` has no `rating`, and the key streams' score cursors and the
/// tag search's score are the whole numbers they were ([`V0Scores`]). `NotificationTypes` is
/// each version's own type, so it needs no patch.
pub fn freeze_docs(docs: OpenApi) -> OpenApi {
    let mut value = serde_json::to_value(&docs).expect("an OpenAPI document serializes");
    if let Some(schemas) = value
        .pointer_mut("/components/schemas")
        .and_then(Value::as_object_mut)
    {
        if let Some(relationships) = schemas.get_mut("PostRelationships") {
            remove_property(relationships, "embedded");
        }
        if let Some(kinds) = schemas.get_mut("PostKind") {
            retain_enum(kinds, |name| {
                serde_json::from_value::<PostKind>(Value::from(name)).is_ok_and(PostKind::is_v0)
            });
        }
        if let Some(types) = schemas.get_mut("NotificationType") {
            retain_enum(types, |name| name != EVENT_CHANGED);
        }
        if let Some(sortings) = schemas.get_mut("StreamSorting") {
            retain_enum(sortings, |name| name != RATING_SORTING);
        }
        if let Some(Value::Array(bodies)) = schemas
            .get_mut("NotificationBody")
            .and_then(|body| body.get_mut("oneOf"))
        {
            bodies.retain(|body| {
                body.pointer("/properties/type/enum")
                    .and_then(Value::as_array)
                    .is_none_or(|tags| !tags.iter().any(|tag| tag == EVENT_CHANGED))
            });
        }
        let cursors = serde_json::to_value(V0Scores::schema()).expect("a schema serializes");
        for (schema, field) in [
            ("ResourceKeyStream", "last_score"),
            ("PostKeyStream", "last_post_score"),
            ("PostsByTagSearch", "score"),
        ] {
            let pointer = format!("/properties/{field}");
            if let (Some(property), Some(frozen)) = (
                schemas
                    .get_mut(schema)
                    .and_then(|stream| stream.pointer_mut(&pointer)),
                cursors.pointer(&pointer),
            ) {
                *property = frozen.clone();
            }
        }
    }
    serde_json::from_value(value).expect("the frozen document reads back")
}

/// `docs` with the description of the response header `header` of `method path`'s `status`
/// answer set to `description`: a header the version sets differently from the shared route
/// annotation. Left as it is if the document has no such header, which the OpenAPI tests pin.
pub fn describe_response_header(
    docs: OpenApi,
    path: &str,
    method: &str,
    status: &str,
    header: &str,
    description: &str,
) -> OpenApi {
    let escape = |segment: &str| segment.replace('~', "~0").replace('/', "~1");
    let pointer = format!(
        "/paths/{}/{method}/responses/{status}/headers/{}/description",
        escape(path),
        escape(header)
    );
    let mut value = serde_json::to_value(&docs).expect("an OpenAPI document serializes");
    if let Some(field) = value.pointer_mut(&pointer) {
        *field = Value::from(description);
    }
    serde_json::from_value(value).expect("the described document reads back")
}

/// `schema` without the property `name`, in its properties and its required list.
fn remove_property(schema: &mut Value, name: &str) {
    if let Some(Value::Object(properties)) = schema.get_mut("properties") {
        properties.remove(name);
    }
    if let Some(Value::Array(required)) = schema.get_mut("required") {
        required.retain(|field| field.as_str() != Some(name));
    }
}

/// `schema`'s `enum` list without the names `keep` refuses.
fn retain_enum(schema: &mut Value, keep: impl Fn(&str) -> bool) {
    if let Some(Value::Array(names)) = schema.get_mut("enum") {
        names.retain(|name| name.as_str().is_some_and(&keep));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// An event kind reads `unknown` in a post's details and a notification's body, a `/v0`
    /// kind or an unrecognised name is left as it is, and an `event_changed` notification
    /// leaves a `notifications` list.
    #[test]
    fn freeze_writes_the_v0_vocabulary() {
        let mut value = json!({
            "posts": [
                {"details": {"kind": "event", "content": "kind"}},
                {"details": {"kind": "short"}},
                {"details": {"kind": "marketplace"}}
            ],
            "notifications": [
                {"timestamp": 1, "body": {"type": "reply", "post_kind": "rsvp"}},
                {"timestamp": 2, "body": {"type": "event_changed", "event_uri": "e"}}
            ]
        });
        freeze(&mut value);
        assert_eq!(
            value,
            json!({
                "posts": [
                    {"details": {"kind": "unknown", "content": "kind"}},
                    {"details": {"kind": "short"}},
                    {"details": {"kind": "marketplace"}}
                ],
                "notifications": [
                    {"timestamp": 1, "body": {"type": "reply", "post_kind": "unknown"}}
                ]
            })
        );
    }

    /// The wrapper writes the stored model through [`freeze`] and documents it under its name.
    #[test]
    fn shape_serializes_frozen_under_the_models_name() {
        use nexus_common::models::post::PostDetails;
        let details = PostDetails {
            kind: PostKind::Availability,
            ..Default::default()
        };
        let stored = serde_json::to_value(&details).unwrap();
        assert_eq!(stored["kind"], "availability");
        let v0 = serde_json::to_value(V0Shape(details)).unwrap();
        assert_eq!(v0["kind"], "unknown");
        assert_eq!(V0Shape::<PostDetails>::name(), PostDetails::name());
    }

    /// `/v0` reads as `unknown` the stored `unknown` and the four event kinds, no other, and the
    /// search's tag alternatives name the same ones.
    #[test]
    fn read_as_unknown_is_unknown_and_the_event_kinds() {
        let kinds = read_as_unknown();
        assert_eq!(
            kinds,
            [
                PostKind::Event,
                PostKind::Rsvp,
                PostKind::Rating,
                PostKind::Availability,
                PostKind::Unknown
            ]
        );
        let mut tags: Vec<&str> = UNKNOWN_SEARCH_TAGS.split('|').collect();
        let mut names: Vec<&str> = kinds.iter().map(|kind| kind.as_str()).collect();
        tags.sort_unstable();
        names.sort_unstable();
        assert_eq!(tags, names);
    }

    /// `/v0`'s `kind=` reads an event kind as `unknown`, like any name it does not know.
    #[test]
    fn v0_kind_reads_event_kinds_as_unknown() {
        for (name, kind) in [
            ("short", PostKind::Short),
            ("collection", PostKind::Collection),
            ("event", PostKind::Unknown),
            ("rsvp", PostKind::Unknown),
            ("rating", PostKind::Unknown),
            ("availability", PostKind::Unknown),
            ("not-a-kind", PostKind::Unknown),
        ] {
            let read: V0PostKind = serde_json::from_value(json!(name)).unwrap();
            assert_eq!(PostKind::from(read), kind, "{name}");
        }
    }

    const AUTHOR: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

    /// A post's relationships lose `embedded`, and an external `replied` reads `null`, at any
    /// depth, also for another app's object whose path nests a post folder; a reply to a post
    /// of either epoch keeps its `replied`.
    #[test]
    fn freeze_writes_the_v0_relationships() {
        let legacy = format!("pubky://{AUTHOR}/pub/pubky.app/posts/00000039YD9BM");
        let social = format!("pubky://{AUTHOR}/pub/social/v1/posts/00000039YD9BM");
        // Another app's object whose path nests a post folder: external, not a post.
        let nested =
            format!("pubky://{AUTHOR}/pub/mapky/objects/pub/social/v1/posts/0033SSE3B1FQ0");
        let mut value = json!({
            "posts": [
                {"relationships": {"replied": "https://example.com/a", "reposted": null,
                    "embedded": "https://example.com/b", "mentioned": []}},
                {"relationships": {"replied": legacy, "reposted": null, "mentioned": [AUTHOR]}},
                {"relationships": {"replied": social, "reposted": null, "mentioned": []}},
                {"relationships": {"replied": nested, "reposted": null, "mentioned": []}}
            ],
            "user": {"relationships": {"embedded": "kept: not a post's relationships"}}
        });
        freeze(&mut value);
        assert_eq!(
            value,
            json!({
                "posts": [
                    {"relationships": {"replied": null, "reposted": null, "mentioned": []}},
                    {"relationships": {"replied": legacy, "reposted": null, "mentioned": [AUTHOR]}},
                    {"relationships": {"replied": social, "reposted": null, "mentioned": []}},
                    {"relationships": {"replied": null, "reposted": null, "mentioned": []}}
                ],
                "user": {"relationships": {"embedded": "kept: not a post's relationships"}}
            })
        );
    }

    /// The wrapper writes the stored model through [`freeze`] and documents it under its name,
    /// while the stored model, the cache's, keeps both fields.
    #[test]
    fn shape_serializes_frozen_relationships() {
        use nexus_common::models::post::{PostRelationships, PostView};
        let view = PostView {
            relationships: PostRelationships {
                replied: Some("https://example.com/a".into()),
                embedded: Some("https://example.com/b".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let stored = serde_json::to_value(&view).unwrap();
        assert_eq!(stored["relationships"]["embedded"], "https://example.com/b");
        assert_eq!(stored["relationships"]["replied"], "https://example.com/a");
        let v0 = serde_json::to_value(V0Shape(view)).unwrap();
        assert!(v0["relationships"].get("embedded").is_none(), "{v0}");
        assert_eq!(v0["relationships"]["replied"], Value::Null);
        assert_eq!(V0Shape::<PostView>::name(), PostView::name());
    }
}
