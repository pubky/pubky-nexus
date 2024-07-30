use super::Followers;
use crate::RedisOps;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the relationship of the user that views and user being viewed.
#[derive(Serialize, Deserialize, ToSchema)]
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
            Some(v_id) => Self::try_from_index(user_id, v_id).await,
        }
    }

    /// Retrieves relationship from Followers/Following Redis index sets.
    pub async fn try_from_index(
        user_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Relationship>, Box<dyn std::error::Error + Send + Sync>> {
        let user_key = [user_id];
        let viewer_key = [viewer_id];
        // Concurrently check if the viewer follows the user and if the user follows the viewer
        let ((user_id_followers_exist, following), (viewer_id_followers_exist, followed_by)) = tokio::try_join!(
            Followers::check_index_set_member(&user_key, viewer_id),
            Followers::check_index_set_member(&viewer_key, user_id)
        )?;

        if user_id_followers_exist && viewer_id_followers_exist {
            // If both sets exist, return the relationship
            return Ok(Some(Self {
                followed_by,
                following,
            }));
        };

        // Run a graph search for followers and populate index sets
        if !user_id_followers_exist {
            Followers::get_from_graph(user_id, None, None).await?;
        }
        if !viewer_id_followers_exist {
            Followers::get_from_graph(viewer_id, None, None).await?;
        }

        // Recheck the relationships after ensuring the data is populated
        let (user_recheck, viewer_recheck) = tokio::try_join!(
            Followers::check_index_set_member(&user_key, viewer_id),
            Followers::check_index_set_member(&viewer_key, user_id)
        )?;
        let (user_exist, following) = user_recheck;
        let (viewer_exist, followed_by) = viewer_recheck;

        if !user_exist || !viewer_exist {
            return Ok(None);
        }

        Ok(Some(Self {
            followed_by,
            following,
        }))
    }
}
