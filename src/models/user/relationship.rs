use crate::models::follow::{Followers, UserFollows};
use crate::models::user::Muted;

use super::UserCounts;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the relationship of the user that views and user being viewed.
#[derive(Serialize, Deserialize, ToSchema, Debug, Default)]
pub struct Relationship {
    pub following: bool,
    pub followed_by: bool,
    pub muted: bool,
}

impl Relationship {
    // Retrieves user-viewer relationship
    pub async fn get_by_id(
        user_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        match viewer_id {
            None => Ok(None),
            Some(v_id) => Self::get_from_index(user_id, v_id).await,
        }
    }

    /// Retrieves relationship from Followers/Following Redis index sets.
    pub async fn get_from_index(
        user_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Relationship>, Box<dyn std::error::Error + Send + Sync>> {
        let user_exist = UserCounts::get_from_index(user_id).await?;
        let viewer_exist = UserCounts::get_from_index(viewer_id).await?;

        // Make sure users exist before get their relationship
        if user_exist.is_none() || viewer_exist.is_none() {
            return Ok(None);
        }

        let (following, followed_by, muted) = tokio::try_join!(
            Followers::check(user_id, viewer_id),
            Followers::check(viewer_id, user_id),
            Muted::check(viewer_id, user_id),
        )?;

        Ok(Some(Self {
            followed_by,
            following,
            muted,
        }))
    }
}
