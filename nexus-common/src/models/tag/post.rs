use crate::db::{queries, RedisOps};
use crate::models::error::ModelResult;
use crate::models::tag::TagDetails;
use crate::types::WotDepth;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::traits::collection::MAX_TAG_PAGE;
use super::traits::{fetch_tag_details, TagCollection, TaggersCollection};

pub const POST_TAGS_KEY_PARTS: [&str; 2] = ["Posts", "Tag"];

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
pub struct TagPost(pub Vec<String>);

impl TagPost {
    /// Tags on a post filtered through the viewer's Web of Trust (graph-only,
    /// uncached). Returns `None` only when the post does not exist; a post with
    /// no trusted-network tags returns `Some(vec![])`. The WoT filter already
    /// bounds the labels to the viewer's trusted taggers, so by default the full
    /// trusted set is returned (`limit_tags` defaults to `MAX_TAG_PAGE`) — this
    /// keeps a trusted moderation tag from being paginated out by tagger count.
    /// `skip_tags`/`limit_taggers` default to 0 / 5; all are capped at
    /// `MAX_TAG_PAGE`.
    pub async fn get_wot_tags_by_post(
        author_id: &str,
        post_id: &str,
        viewer_id: &str,
        depth: WotDepth,
        skip_tags: Option<usize>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
    ) -> ModelResult<Option<Vec<TagDetails>>> {
        let query = queries::get::get_viewer_trusted_network_post_tags(
            author_id,
            post_id,
            viewer_id,
            depth,
            skip_tags.unwrap_or(0),
            limit_tags.unwrap_or(MAX_TAG_PAGE).min(MAX_TAG_PAGE),
            limit_taggers.unwrap_or(5).min(MAX_TAG_PAGE),
        );
        Ok(fetch_tag_details(query).await?)
    }
}

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
