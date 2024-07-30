use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::tag::user::{ UserTag, UserTags };
use super::{UserCounts, UserDetails, Relationship};

/// Represents a Pubky user with relational data including tags, counts, bookmark and relationship with other posts.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserView {
    // TODO#35: Think if this is good approach
    pub details: UserDetails,
    pub counts: UserCounts,
    pub tags: Vec<UserTag>,
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
            tags: vec![],
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

        // TODO#35: Error control
        let tags = match tags {
            None => return Ok(None),
            Some(user_tags) => user_tags,
        };

        Ok(Some(Self {
            details,
            counts,
            viewer,
            tags
        }))
    }
}
