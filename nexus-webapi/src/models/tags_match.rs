//! The `match` parameter of the resource streams: how `/v1` matches several `tags`.
//!
//! `/v1` reads [`TagsMatch`], `any` (the default) or `all`, the names and the meaning Pinakes
//! gives its own `match`, so a client that falls back from Pinakes to Nexus sends the same
//! query. `/v0` is frozen and reads [`NoTagsMatch`]: no parameter, always `any`.

use nexus_common::models::resource::stream::TagMatch;
use serde::de::IgnoredAny;
use serde::Deserialize;
use serde_json::Value;
use utoipa::openapi::path::{Parameter, ParameterBuilder, ParameterIn};
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::Required;
use utoipa::IntoParams;

/// `match=` on `/v1`'s resource streams. Any name but `any` and `all` is a 400.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(transparent)]
pub struct TagsMatch(pub TagMatch);

impl From<TagsMatch> for TagMatch {
    fn from(tags_match: TagsMatch) -> Self {
        tags_match.0
    }
}

impl IntoParams for TagsMatch {
    fn into_params(_: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        vec![ParameterBuilder::new()
            .name("match")
            .parameter_in(ParameterIn::Query)
            .description(Some(format!(
                "How several `tags` match: `any` (default) keeps a resource tagged with at least \
                 one of them, `all` only a resource tagged with every one, by any taggers. A \
                 label listed twice counts once. `all` takes 1-{} distinct labels and needs \
                 `tags`; otherwise 400.",
                TagMatch::MAX_ALL_LABELS
            )))
            .required(Required::False)
            .schema(Some(
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .enum_values(Some(["any", "all"]))
                    .default(Some(Value::String("any".to_string()))),
            ))
            .build()]
    }
}

/// What `/v0` reads where `/v1` reads `match`: nothing. Its resource streams always match
/// `any`, document no parameter, and read a `match` as any unknown parameter: ignored.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq)]
#[serde(transparent)]
pub struct NoTagsMatch(IgnoredAny);

impl From<NoTagsMatch> for TagMatch {
    fn from(_: NoTagsMatch) -> Self {
        TagMatch::Any
    }
}

impl IntoParams for NoTagsMatch {
    fn into_params(_: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        Vec::new()
    }
}
