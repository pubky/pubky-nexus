pub mod error_response;
pub mod info;
pub mod post;

pub mod bounded_vec;
pub mod global_post_id;
pub mod post_id;
pub mod pubky_id;
pub mod resource_id;
pub mod tag_label;
pub mod user_id_prefix;
pub mod username_prefix;

use std::ops::Deref;

use serde::Deserialize;
use utoipa::ToSchema;

pub use error_response::ErrorResponse;
pub use global_post_id::{GlobalPostId, GlobalPostIds};
pub use info::ServerInfo;
pub use post::{PostStreamDetailed, PostViewDetailed};
pub use post_id::{PostId, PostIds};
pub use pubky_id::PubkyId;
pub use resource_id::ResourceId;
pub use tag_label::TagLabel;
pub use user_id_prefix::UserIdPrefix;
pub use username_prefix::UsernamePrefix;

/// Comma-separated list of tag strings (min=1, max=5).
#[derive(Debug, ToSchema)]
pub struct Tags(pub Vec<String>);

impl Deref for Tags {
    type Target = Vec<String>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Tags {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_vec::deserialize_csv::<String, D, 1, 5>(d).map(Self)
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

impl<'de> Deserialize<'de> for UserIds {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_vec::deserialize_json_array::<PubkyId, D, 1, 100>(d).map(Self)
    }
}
