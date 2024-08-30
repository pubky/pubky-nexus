use crate::RedisOps;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::traits::TagCollection;

const USER_TAGS_KEY_PARTS: [&str; 2] = ["Users", "Tag"];

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
pub struct TagUser;

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
}
