use crate::RedisOps;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{traits::TagCollection, TagDetails};

const USER_TAGS_KEY_PARTS: [&str; 2] = ["Users", "Tag"];

// Define a newtype wrapper
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

impl TagUser {
    pub async fn get_by_id(
        user_id: &str,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
    ) -> Result<Option<Vec<TagDetails>>, Box<dyn std::error::Error + Send + Sync>> {
        Self::get_collection(user_id, None, limit_tags, limit_taggers).await
    }
}
