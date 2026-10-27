//! `/v0` is frozen: what the stored models gained since stays out of it.
//!
//! External post references give a post's stored relationships an `embedded` URI, an external
//! embed, and let `replied` name an external resource. The cache keeps both, and `/v1` serves
//! them. `/v0` serves what it served before: no `embedded`, and a `replied` only when it names
//! a post, `null` otherwise, as its graph read answered for a reply to anything but a post.
//!
//! The responses `/v0` shares with `/v1` that carry a post's relationships are written through
//! [`V0Shape`], which serializes the stored model and then takes the additions out
//! ([`freeze`]); [`freeze_docs`] does the same to the `/v0` document's shared schemas.

use std::borrow::Cow;

use serde::ser::Serializer;
use serde::Serialize;
use serde_json::Value;
use utoipa::openapi::{OpenApi, RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

/// A `/v0` response: the stored model `T`, written as `/v0` has always written it
/// ([`freeze`]). Documented as `T` is, under `T`'s name, so the `/v0` document keeps its
/// references.
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

/// Takes what `/v0` does not have out of a serialized stored model, at any depth: a post's
/// `relationships` lose `embedded`, and a `replied` that names no post reads `null`
/// ([`freeze_relationships`]). Every other value is left as it is.
pub fn freeze(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                if key == "relationships" {
                    freeze_relationships(field);
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

/// Whether `uri` is a post's stored address, in either epoch's folder. Only a post's address
/// is, so this tells a reply to a post from a reply to an external resource (another scheme's
/// URL or another app's object) without parsing, and so without refusing a post whose id an
/// older writer spelled loosely.
fn is_post_uri(uri: &str) -> bool {
    uri.starts_with("pubky://") && POST_FOLDERS.iter().any(|folder| uri.contains(folder))
}

/// The folders a post's stored address names, `pubky.app`'s and `social/v1`'s.
const POST_FOLDERS: [&str; 2] = ["/pub/pubky.app/posts/", "/pub/social/v1/posts/"];

/// The `/v0` document as `/v0` has always been documented: the shared schemas the stored
/// models document lose what was added since. `PostRelationships` has no `embedded`.
pub fn freeze_docs(docs: OpenApi) -> OpenApi {
    let mut value = serde_json::to_value(&docs).expect("an OpenAPI document serializes");
    if let Some(schemas) = value
        .pointer_mut("/components/schemas")
        .and_then(Value::as_object_mut)
    {
        if let Some(relationships) = schemas.get_mut("PostRelationships") {
            remove_property(relationships, "embedded");
        }
    }
    serde_json::from_value(value).expect("the frozen document reads back")
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const AUTHOR: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";

    /// A post's relationships lose `embedded`, and an external `replied` reads `null`, at any
    /// depth; a reply to a post of either epoch keeps its `replied`.
    #[test]
    fn freeze_writes_the_v0_relationships() {
        let legacy = format!("pubky://{AUTHOR}/pub/pubky.app/posts/00000039YD9BM");
        let social = format!("pubky://{AUTHOR}/pub/social/v1/posts/00000039YD9BM");
        let mut value = json!({
            "posts": [
                {"relationships": {"replied": "https://example.com/a", "reposted": null,
                    "embedded": "https://example.com/b", "mentioned": []}},
                {"relationships": {"replied": legacy, "reposted": null, "mentioned": [AUTHOR]}},
                {"relationships": {"replied": social, "reposted": null, "mentioned": []}}
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
                    {"relationships": {"replied": social, "reposted": null, "mentioned": []}}
                ],
                "user": {"relationships": {"embedded": "kept: not a post's relationships"}}
            })
        );
    }

    /// The wrapper writes the stored model through [`freeze`] and documents it under its name,
    /// while the stored model, the cache's, keeps both fields.
    #[test]
    fn shape_serializes_frozen_under_the_models_name() {
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
