use std::ops::Deref;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use crate::RedisOps;

pub mod user;

impl RedisOps for Tags {}

// Atomic struct to save in the cache
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tag {
    tag_id: String, // TODO: Crobfordbase32 type
    indexed_at: i64,
    tagger_id: String, // TODO#35: or maybe user_id name?
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

// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tags(Vec<Tag>);

impl Default for Tags {
    fn default() -> Self {
        Tags(Vec::new())
    }
}

// Implement Deref so TagList can be used like Vec<String>
impl Deref for Tags {
    type Target = Vec<Tag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
