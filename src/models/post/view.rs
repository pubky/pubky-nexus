use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Bookmark, PostCounts, PostDetails, PostRelationships, PostTags};

/// Represents a Pubky user profile with relational data including tags, counts, and relationship with a viewer.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostView {
    details: PostDetails,
    counts: PostCounts,
    tags: PostTags,
    relationships: PostRelationships,
    bookmark: Option<Bookmark>,
}

impl Default for PostView {
    fn default() -> Self {
        Self::new()
    }
}

impl PostView {
    pub fn new() -> Self {
        Self {
            details: PostDetails::new(),
            counts: PostCounts::new(),
            tags: PostTags::new(),
            relationships: PostRelationships::new(),
            bookmark: Some(Bookmark::new()),
        }
    }

    /// Retrieves a profile by user ID, checking the cache first and then the graph database.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        // Perform all operations concurrently
        let (details, counts, bookmark) = tokio::try_join!(
            PostDetails::get_by_id(author_id, post_id),
            PostCounts::get_by_id(author_id, post_id),
            Bookmark::get_by_id(author_id, post_id, viewer_id), // PostRelationships::get_by_id(author_id, post_id),
        )?;

        let details = match details {
            None => return Ok(None),
            Some(details) => details,
        };

        let counts = counts.unwrap_or_default();
        //let relationships = relationships.unwrap_or_default();

        Ok(Some(Self {
            details,
            counts,
            bookmark,
            relationships: PostRelationships::new(),
            tags: PostTags::new(), //TODO
        }))
    }
}
