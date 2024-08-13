use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use utoipa::ToSchema;

pub mod post;
pub mod user;
// Atomic struct to save in the cache
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct Tag {
    tag_id: String, // TODO: Crobfordbase32 type
    indexed_at: i64,
    tagger_id: String,
}

impl Default for Tag {
    fn default() -> Self {
        Self {
            tag_id: String::new(),
            indexed_at: Utc::now().timestamp(),
            tagger_id: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
pub struct Tags(Vec<Tag>);

// Implement Deref so TagList can be used like Vec<String>
impl Deref for Tags {
    type Target = Vec<Tag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
