use std::error::Error;

use super::{Followers, Following, UserView};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize)]
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
        skip: Option<usize>,
        limit: Option<usize>,
        list_type: UserStreamType,
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        let user_ids = match list_type {
            UserStreamType::Followers => Followers::get_by_id(user_id, skip, limit)
                .await?
                .map(|followers| followers.0)
                .unwrap_or_default(),
            UserStreamType::Following => Following::get_by_id(user_id, skip, limit)
                .await?
                .map(|following| following.0)
                .unwrap_or_default(),
        };
        Self::from_listed_user_ids(&user_ids).await
    }

    pub async fn from_listed_user_ids(
        user_ids: &[String],
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let mut user_views = Vec::with_capacity(user_ids.len());
        for user_id in user_ids {
            if let Some(user_view) = UserView::get_by_id(user_id, None).await? {
                user_views.push(user_view);
            }
        }
        Ok(Some(Self(user_views)))
    }
}
