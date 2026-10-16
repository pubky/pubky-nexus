use crate::db::{fetch_key_from_graph, queries};
use crate::models::error::ModelResult;
use crate::models::follow::{Followers, UserFollows};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the relationship of the user that views and user being viewed.
#[derive(Serialize, Deserialize, ToSchema, Debug, Default)]
pub struct Relationship {
    pub following: bool,
    pub followed_by: bool,
}

impl Relationship {
    /// `None` without a viewer or when either user has no graph node.
    pub async fn get_by_id(user_id: &str, viewer_id: Option<&str>) -> ModelResult<Option<Self>> {
        let Some(viewer_id) = viewer_id else {
            return Ok(None);
        };
        let both_exist = fetch_key_from_graph(
            queries::get::users_exist(&[user_id, viewer_id]),
            "all_exist",
        )
        .await?
        .unwrap_or(false);
        if !both_exist {
            return Ok(None);
        }

        // Follow membership still comes from the Followers sets.
        let (following, followed_by) = tokio::try_join!(
            Followers::check_in_index(user_id, viewer_id),
            Followers::check_in_index(viewer_id, user_id),
        )?;

        Ok(Some(Self {
            followed_by,
            following,
        }))
    }
}
