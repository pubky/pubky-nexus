pub mod error_response;
pub mod info;
pub mod post;

pub mod bounded_limit;
pub mod bounded_pagination;
pub mod bounded_skip;
pub mod bounded_vec;
pub mod crockford_id;
pub mod global_post_id;
pub mod post_search_query;
pub mod resource_id;
pub mod tag_label;
pub mod user_id_prefix;
pub mod username_prefix;

use std::ops::Deref;

use serde::Deserialize;
use utoipa::ToSchema;

pub use bounded_limit::BoundedLimit;
pub use bounded_pagination::BoundedPagination;
pub use bounded_skip::BoundedSkip;
pub use crockford_id::{FileId, FileUris, PostId};
pub use error_response::ErrorResponsePayload;
pub use global_post_id::{GlobalPostId, GlobalPostIds};
pub use info::ServerInfo;
pub use post::{PostStreamDetailed, PostViewDetailed};
pub use post_search_query::PostSearchQuery;
pub use pubky_app_specs::{PubkyAppPostKind, PubkyId};
pub use resource_id::ResourceId;
pub use tag_label::TagLabel;
pub use user_id_prefix::UserIdPrefix;
pub use username_prefix::UsernamePrefix;

/// Comma-separated list of tag labels (min=1, max=5). Each label is validated and sanitized.
#[derive(Debug, ToSchema)]
#[schema(value_type = String, example = "dev,free,opensource")]
pub struct Tags(pub Vec<TagLabel>);

impl Deref for Tags {
    type Target = Vec<TagLabel>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Tags {
    pub fn to_string_vec(&self) -> Vec<String> {
        self.iter().map(|t| t.0.clone()).collect()
    }
}

impl<'de> Deserialize<'de> for Tags {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_vec::deserialize_csv::<TagLabel, D, 1, 5>(d).map(Self)
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
