use crate::RedisOps;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// TODO: Decide a better namimg, DO not like
use super::traits::TagCollection;
use super::traits::TaggersCollection;

pub const POST_TAGS_KEY_PARTS: [&str; 2] = ["Posts", "Tag"];

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
pub struct TagPost(pub Vec<String>);

impl AsRef<[String]> for TagPost {
    fn as_ref(&self) -> &[String] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for TagPost {
    async fn prefix() -> String {
        String::from("Post:Taggers")
    }
}

impl TagCollection for TagPost {
    fn get_tag_prefix<'a>() -> [&'a str; 2] {
        POST_TAGS_KEY_PARTS
    }
}

impl TaggersCollection for TagPost {}
