use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Relationship, UserCounts, UserDetails};
use crate::models::tag::traits::TagCollection;
use crate::models::tag::user::TagUser;
use crate::models::tag::TagDetails;
use crate::types::DynError;

/// Represents a Pubky user with relational data including tags, counts, bookmark and relationship with other posts.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct UserView {
    pub details: UserDetails,
    pub counts: UserCounts,
    pub tags: Vec<TagDetails>,
    pub relationship: Relationship,
}

impl UserView {
    /// Retrieves a user by ID, checking the cache first and then the graph database.
    pub async fn get_by_id(
        user_id: &str,
        viewer_id: Option<&str>,
        depth: Option<u8>,
    ) -> Result<Option<Self>, DynError> {
        // Perform all operations concurrently
        let (details, counts, relationship, tags) = tokio::try_join!(
            UserDetails::get_by_id(user_id),
            UserCounts::get_by_id(user_id),
            Relationship::get_by_id(user_id, viewer_id),
            TagUser::get_by_id(user_id, None, None, None, viewer_id, depth)
        )?;

        let details = match details {
            None => return Ok(None),
            Some(details) => details,
        };

        let counts = counts.unwrap_or_default();
        let relationship = relationship.unwrap_or_default();
        let tags = tags.unwrap_or_default();

        Ok(Some(Self {
            details,
            counts,
            relationship,
            tags,
        }))
    }
}
