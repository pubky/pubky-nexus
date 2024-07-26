//use std::ops::Deref;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use crate::RedisOps;

pub mod user;

impl RedisOps for Tag {}
//impl RedisOps for TagList {}

#[derive(Serialize, Deserialize, Debug)]
pub struct Tag {
    tag_id: String, // TODO: Crobfordbase32 type
    indexed_at: i64,
    tagger_id: String, // TODO#: or maybe user_id name?
}

impl Default for Tag {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a profile link with a title and URL.
impl Tag {
    pub fn new() -> Self {
        Self {
            tag_id: String::new(),
            indexed_at: Utc::now().timestamp(),
            tagger_id: String::new(),
        }
    }

    pub async fn type_name() -> String {
        Self::prefix().await
    }
}

// // Define a newtype wrapper
// #[derive(Serialize, Deserialize, Debug)]
// struct TagList(Vec<Tag>);

// // Implement Deref so TagList can be used like Vec<String>
// impl Deref for TagList {
//     type Target = Vec<Tag>;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

