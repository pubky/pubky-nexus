use std::error::Error;

use super::{Followers, Following, UserView};
use serde::{Deserialize, Serialize};
use tokio::task::spawn;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub enum UserStreamType {
    Followers,
    Following,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct UserStream(Vec<UserView>);

impl Default for UserStream {
    fn default() -> Self {
        Self::new()
    }
}

impl UserStream {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub async fn get_by_id(
        user_id: &str,
        viewer_id: Option<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
        list_type: UserStreamType,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let user_ids = match list_type {
            UserStreamType::Followers => Followers::get_by_id(user_id, skip, limit)
                .await?
                .map(|followers| followers.0),
            UserStreamType::Following => Following::get_by_id(user_id, skip, limit)
                .await?
                .map(|following| following.0),
        };
        match user_ids {
            Some(users) => Self::from_listed_user_ids(&users, viewer_id).await,
            None => Ok(None),
        }
    }

    pub async fn from_listed_user_ids(
        user_ids: &[String],
        viewer_id: Option<&str>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        // TODO: potentially we could use a new redis_com.mget() with a single call to retrieve all
        // user details at once and build the user profiles on the fly.
        // But still, using tokio to create them concurrently has VERY high performance.
        let viewer_id = viewer_id.map(|id| id.to_string());
        let mut handles = Vec::with_capacity(user_ids.len());

        for user_id in user_ids {
            let user_id = user_id.clone();
            let viewer_id = viewer_id.clone();
            let handle =
                spawn(async move { UserView::get_by_id(&user_id, viewer_id.as_deref()).await });
            handles.push(handle);
        }

        let mut user_views = Vec::with_capacity(user_ids.len());

        for handle in handles {
            if let Some(user_view) = handle.await?? {
                user_views.push(user_view);
            }
        }

        Ok(Some(Self(user_views)))
    }
}
