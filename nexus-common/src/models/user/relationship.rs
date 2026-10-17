use crate::db::kv::RedisResult;
use crate::models::error::ModelResult;
use crate::models::follow::{Followers, UserFollows};

use super::users_exist;
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
        if !users_exist(&[user_id, viewer_id]).await? {
            return Ok(None);
        }
        Ok(Some(Self::from_follow_sets(user_id, viewer_id).await?))
    }

    /// Follow membership from the Followers sets. Callers settle that both
    /// users exist first; a page does that once, not per row.
    pub async fn from_follow_sets(user_id: &str, viewer_id: &str) -> RedisResult<Self> {
        let (following, followed_by) = tokio::try_join!(
            Followers::check_in_index(user_id, viewer_id),
            Followers::check_in_index(viewer_id, user_id),
        )?;
        Ok(Self {
            followed_by,
            following,
        })
    }
}
