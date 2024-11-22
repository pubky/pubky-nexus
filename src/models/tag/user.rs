use crate::RedisOps;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::traits::{TagCollection, TaggersCollection};

pub const USER_TAGS_KEY_PARTS: [&str; 2] = ["Users", "Tag"];
pub const USER_DISTANCE_TAGS_KEY_PARTS: [&str; 3] = ["Cache", "Users", "Tag"];

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
pub struct TagUser(pub Vec<String>);

impl AsRef<[String]> for TagUser {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for TagUser {
    async fn prefix() -> String {
        String::from("User:Taggers")
    }
}

impl TagCollection for TagUser {
    fn get_tag_prefix<'a>() -> [&'a str; 2] {
        USER_TAGS_KEY_PARTS
    }

    fn get_cache_tag_prefix<'a>() -> [&'a str; 3] {
        USER_DISTANCE_TAGS_KEY_PARTS
    }
}

impl TaggersCollection for TagUser {}
