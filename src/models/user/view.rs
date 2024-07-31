use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Relationship, UserCounts, UserDetails};
use crate::models::tag::user::UserTags;

/// Represents a Pubky user with relational data including tags, counts, bookmark and relationship with other posts.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserView {
    details: UserDetails,
    counts: UserCounts,
    pub tags: UserTags,
    viewer: Relationship,
}

impl Default for UserView {
    fn default() -> Self {
        Self::new()
    }
}

impl UserView {
    pub fn new() -> Self {
        Self {
            details: UserDetails::new(),
            counts: UserCounts::new(),
            tags: UserTags::default(),
            viewer: Relationship::new(),
        }
    }

    /// Retrieves a user by ID, checking the cache first and then the graph database.
    pub async fn get_by_id(
        user_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        // Perform all operations concurrently
        let (details, counts, viewer, tags) = tokio::try_join!(
            UserDetails::get_by_id(user_id),
            UserCounts::get_by_id(user_id),
            Relationship::get_by_id(user_id, viewer_id),
            UserTags::get_by_id(user_id)
        )?;

        let details = match details {
            None => return Ok(None),
            Some(details) => details,
        };

        let counts = counts.unwrap_or_default();
        let viewer = viewer.unwrap_or_default();

        let tags = tags.unwrap_or_default();

        Ok(Some(Self {
            details,
            counts,
            viewer,
            tags,
        }))
    }
}
