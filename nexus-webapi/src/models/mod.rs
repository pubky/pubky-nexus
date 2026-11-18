pub mod error_response;
pub mod info;
pub mod post;

pub mod bounded_limit;
pub mod bounded_pagination;
pub mod bounded_skip;
pub mod bounded_vec;
pub mod crockford_id;
pub mod file_ref;
pub mod global_post_id;
pub mod notification_types;
pub mod post_kinds;
pub mod post_search_query;
pub mod resource_id;
pub mod tag_label;
pub mod user_id_prefix;
pub mod username_prefix;
pub mod v1;

use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;

use serde::Deserialize;
use serde_json::Value;
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

pub use bounded_limit::BoundedLimit;
pub use bounded_pagination::BoundedPagination;
pub use bounded_skip::BoundedSkip;
pub use crockford_id::{FileId, FileUris, PostId};
pub use error_response::ErrorResponsePayload;
pub use file_ref::FileRef;
pub use global_post_id::{GlobalPostId, GlobalPostIds};
pub use info::ServerInfo;
pub use nexus_common::models::post::PostKind;
pub use notification_types::NotificationTypes;
pub use post::{IncludeAttachmentMetadata, PostStreamDetailed, PostViewDetailed};
pub use post_kinds::{KindToken, PostKinds, StoredKindToken, StoredPostKinds};
pub use post_search_query::PostSearchQuery;
pub use pubky_social_specs::PubkyId;
pub use resource_id::ResourceId;
pub use tag_label::TagLabel;
pub use user_id_prefix::UserIdPrefix;
pub use username_prefix::UsernamePrefix;

/// Comma-separated list of tag labels (min=1, max=5). Each label is validated and sanitized.
///
/// `L` is the version's label type, `/v0`'s [`TagLabel`] or `/v1`'s [`v1::TagLabel`], so each
/// label of the list folds and validates by the version's rule. Documented as one string schema
/// named `Tags` on both versions, by hand: the derive would name a reference written with its
/// argument `Tags_TagLabel`.
#[derive(Debug)]
pub struct Tags<L>(pub Vec<L>);

impl<L> Deref for Tags<L> {
    type Target = Vec<L>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The labels of a list as the queries take them, whichever version's rule folded them.
pub trait TagLabels {
    /// The labels as plain strings, in the order written.
    fn to_string_vec(&self) -> Vec<String>;
}

impl<L: Deref<Target = str>> TagLabels for Tags<L> {
    fn to_string_vec(&self) -> Vec<String> {
        self.iter().map(|label| label.to_string()).collect()
    }
}

impl<'de, L> Deserialize<'de> for Tags<L>
where
    L: TryFrom<String>,
    L::Error: fmt::Display,
{
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_vec::deserialize_csv::<L, D, 1, 5>(d).map(Self)
    }
}

impl<L> PartialSchema for Tags<L> {
    fn schema() -> RefOr<Schema> {
        // `example`, not `examples`: the key the derive writes, and the frozen `/v0` document
        // carries.
        #[allow(deprecated)]
        ObjectBuilder::new()
            .schema_type(Type::String)
            .description(Some(
                "Comma-separated list of tag labels (min=1, max=5). Each label is validated and sanitized.",
            ))
            .example(Some(Value::String("dev,free,opensource".to_string())))
            .into()
    }
}

impl<L> ToSchema for Tags<L> {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("Tags")
    }
}

/// JSON array of `PubkyId` values (min=1, max=100).
#[derive(Debug, ToSchema)]
pub struct UserIds(pub Vec<PubkyId>);

impl Deref for UserIds {
    type Target = Vec<PubkyId>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl UserIds {
    pub fn to_string_vec(&self) -> Vec<String> {
        self.iter().map(|id| id.to_string()).collect()
    }
}

impl<'de> Deserialize<'de> for UserIds {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_vec::deserialize_json_array::<PubkyId, D, 1, 100>(d).map(Self)
    }
}

/// What the unit tests of the versioned shapes share.
#[cfg(test)]
pub(crate) mod test_helpers {
    use serde_json::Value;
    use utoipa::PartialSchema;

    /// The `enum` values of a schema, in the schema's order.
    pub(crate) fn enum_values<T: PartialSchema>() -> Vec<String> {
        let schema = serde_json::to_value(T::schema()).unwrap();
        schema["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("no enum in {schema}"))
            .iter()
            .map(|name| name.as_str().unwrap().to_string())
            .collect()
    }

    /// Every key of `value` named `name`, at any depth.
    pub(crate) fn keys_named(value: &Value, name: &str) -> usize {
        match value {
            Value::Object(fields) => fields
                .iter()
                .map(|(key, value)| usize::from(key == name) + keys_named(value, name))
                .sum(),
            Value::Array(items) => items.iter().map(|item| keys_named(item, name)).sum(),
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;

    /// The list as a query string carries it.
    fn parse<L>(csv: &str) -> Result<Tags<L>, serde_json::Error>
    where
        Tags<L>: DeserializeOwned,
    {
        serde_json::from_str(&format!("\"{csv}\""))
    }

    /// The `/v0` list folds each label by the frozen `pubky.app` rule: the surrounding
    /// whitespace trimmed and every letter lowercased, a non-ASCII one included.
    #[test]
    fn v0_list_folds_each_label_by_the_v0_rule() {
        let labels = parse::<TagLabel>(" Dev , FREE,Ärger").unwrap();
        assert_eq!(labels.to_string_vec(), ["dev", "free", "ärger"]);
        assert_eq!(labels.len(), 3);
    }

    /// One invalid label rejects the whole list, on the `/v0` rule.
    #[test]
    fn v0_list_rejects_an_invalid_label() {
        assert!(parse::<TagLabel>("dev,co ol").is_err());
        assert!(parse::<TagLabel>(&format!("dev,{}", "a".repeat(21))).is_err());
        assert!(parse::<TagLabel>("dev,a:b").is_err());
    }

    /// One to five labels, whichever label type the list holds.
    #[test]
    fn list_takes_one_to_five_labels() {
        assert!(parse::<TagLabel>("").is_err());
        assert!(parse::<TagLabel>("a,b,c,d,e,f").is_err());
        assert_eq!(parse::<TagLabel>("a,b,c,d,e").unwrap().len(), 5);
        assert!(parse::<v1::TagLabel>("").is_err());
        assert!(parse::<v1::TagLabel>("a,b,c,d,e,f").is_err());
        assert_eq!(parse::<v1::TagLabel>("a,b,c,d,e").unwrap().len(), 5);
    }

    /// Both versions' lists document one string schema named `Tags`, the same on each.
    #[test]
    fn schema_is_one_string_named_tags_on_both_versions() {
        assert_eq!(<Tags<TagLabel> as ToSchema>::name(), "Tags");
        assert_eq!(<v1::Tags as ToSchema>::name(), "Tags");
        let v0 = serde_json::to_value(<Tags<TagLabel> as PartialSchema>::schema()).unwrap();
        let v1 = serde_json::to_value(<v1::Tags as PartialSchema>::schema()).unwrap();
        assert_eq!(v0, v1);
        assert_eq!(v0["type"], "string", "{v0}");
        assert_eq!(v0["example"], "dev,free,opensource", "{v0}");
    }
}
