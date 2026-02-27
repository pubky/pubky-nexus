use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Bookmark, PostCounts, PostDetails, PostRelationships};
use crate::models::error::ModelResult;
use crate::models::tag::post::TagPost;
use crate::models::tag::traits::TagCollection;
use crate::models::tag::TagDetails;

/// Represents a Pubky user with relational data including tags, counts, and relationship with a viewer.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct PostView {
    pub details: PostDetails,
    pub counts: PostCounts,
    pub tags: Vec<TagDetails>,
    pub relationships: PostRelationships,
    pub bookmark: Option<Bookmark>,
}

impl PostView {
    /// Retrieves a user ID, checking the cache first and then the graph database.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        viewer_id: Option<&str>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
    ) -> ModelResult<Option<Self>> {
        // Perform all operations concurrently
        let (details, counts, bookmark, relationships) = tokio::try_join!(
            PostDetails::get_by_id(author_id, post_id),
            PostCounts::get_by_id(author_id, post_id),
            Bookmark::get_by_id(author_id, post_id, viewer_id),
            PostRelationships::get_by_id(author_id, post_id),
        )?;

        let details = match details {
            None => return Ok(None),
            Some(details) => details,
        };

        let counts = counts.unwrap_or_default();
        let relationships = relationships.unwrap_or_default();

        // Before fetching post tags, check if the post has any tags
        // Without this check, the index search will return a NONE because the tag index
        // doesn't exist, leading us to query the graph unnecessarily, assuming the data wasn't indexed
        let tags = match counts.tags {
            0 => Vec::new(),
            _ => {
                TagPost::get_by_id(
                    author_id,
                    Some(post_id),
                    None,
                    limit_tags,
                    limit_taggers,
                    viewer_id,
                    None, // Avoid by default WoT tags in a Post
                )
                .await?
                .unwrap_or_default()
            }
        };

        Ok(Some(Self {
            details,
            counts,
            bookmark,
            relationships,
            tags,
        }))
    }
}
