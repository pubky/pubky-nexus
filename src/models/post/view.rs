use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Bookmark, PostCounts, PostDetails, PostRelationships};
use crate::models::tag::post::TagPost;
use crate::models::tag::traits::TagCollection;
use crate::models::tag::TagDetails;

/// Represents a Pubky user with relational data including tags, counts, and relationship with a viewer.
#[derive(Serialize, Deserialize, ToSchema, Default)]
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
        max_tags: Option<usize>,
        max_taggers: Option<usize>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        // Perform all operations concurrently
        let (details, counts, bookmark, relationships, tags) = tokio::try_join!(
            PostDetails::get_by_id(author_id, post_id),
            PostCounts::get_by_id(author_id, post_id),
            Bookmark::get_by_id(author_id, post_id, viewer_id),
            PostRelationships::get_by_id(author_id, post_id),
            TagPost::try_from_multiple_index(author_id, Some(post_id), max_tags, max_taggers),
        )?;

        let details = match details {
            None => return Ok(None),
            Some(details) => details,
        };

        let counts = counts.unwrap_or_default();
        let relationships = relationships.unwrap_or_default();

        let tags = tags.unwrap_or_default();

        Ok(Some(Self {
            details,
            counts,
            bookmark,
            relationships,
            tags,
        }))
    }
}
