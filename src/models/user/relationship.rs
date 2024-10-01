use super::Followers;
use crate::RedisOps;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the relationship of the user that views and user being viewed.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct Relationship {
    pub following: bool,
    pub followed_by: bool,
}

impl Default for Relationship {
    fn default() -> Self {
        Self::new()
    }
}

impl Relationship {
    pub fn new() -> Self {
        Self {
            following: false,
            followed_by: false,
        }
    }

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
        let user_key = [user_id];
        let viewer_key = [viewer_id];
        // Concurrently check if the viewer follows the user and if the user follows the viewer
        let ((_user_id_followers_exist, following), (_viewer_id_followers_exist, followed_by)) = tokio::try_join!(
            Followers::check_set_member(&user_key, viewer_id),
            Followers::check_set_member(&viewer_key, user_id)
        )?;

        Ok(Some(Self {
            followed_by,
            following,
        }))
    }
}
