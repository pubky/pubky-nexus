use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Relationship, UserCounts, UserDetails};
use crate::db::RedisOps;
use crate::models::error::ModelResult;
use crate::models::tag::traits::TagCollection;
use crate::models::tag::user::TagUser;
use crate::models::tag::TagDetails;

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
    ) -> ModelResult<Option<Self>> {
        // Perform all operations concurrently
        let (details, counts, relationship) = tokio::try_join!(
            UserDetails::get_by_id(user_id),
            UserCounts::get_by_id(user_id),
            Relationship::get_by_id(user_id, viewer_id),
        )?;

        let Some(details) = details else {
            return Ok(None);
        };
        let counts = counts.unwrap_or_default();
        let relationship = relationship.unwrap_or_default();

        // Before fetching post tags, check if the post has any tags
        // Without this check, the index search will return a NONE because the tag index
        // doesn't exist, leading us to query the graph unnecessarily, assuming the data wasn't indexed
        let tags = match counts.tags {
            0 => Vec::new(),
            _ => TagUser::get_by_id(user_id, None, None, None, None, viewer_id, depth)
                .await?
                .unwrap_or_default(),
        };

        Ok(Some(Self {
            details,
            counts,
            relationship,
            tags,
        }))
    }

    /// Retrieves multiple users by their IDs using batch Redis operations for better performance.
    ///
    /// This method uses the new `mget` operation to fetch user details and counts in bulk,
    /// significantly improving performance when retrieving multiple users.
    pub async fn get_by_ids(
        user_ids: &[String],
        viewer_id: Option<&str>,
        depth: Option<u8>,
    ) -> ModelResult<Vec<Option<Self>>> {
        // Use mget to fetch all user details and counts in bulk
        let (details_list, counts_list): (Vec<Option<UserDetails>>, Vec<Option<UserCounts>>) =
            tokio::try_join!(UserDetails::mget(user_ids), UserCounts::mget(user_ids))?;

        let mut user_views = Vec::with_capacity(user_ids.len());

        for (i, user_id) in user_ids.iter().enumerate() {
            let details = &details_list[i];
            let counts = &counts_list[i];

            let Some(details) = details else {
                user_views.push(None);
                continue;
            };

            let counts = counts.clone().unwrap_or_default();
            let relationship = Relationship::get_by_id(user_id, viewer_id)
                .await?
                .unwrap_or_default();

            // Before fetching post tags, check if the post has any tags
            let tags = match counts.tags {
                0 => Vec::new(),
                _ => TagUser::get_by_id(user_id, None, None, None, None, viewer_id, depth)
                    .await?
                    .unwrap_or_default(),
            };

            user_views.push(Some(Self {
                details: details.clone(),
                counts,
                relationship,
                tags,
            }));
        }

        Ok(user_views)
    }
}
