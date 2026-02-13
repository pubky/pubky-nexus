use crate::db::RedisOps;
use crate::models::error::ModelResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::followers::Followers;
use super::following::Following;
use super::traits::UserFollows;

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct Friends(pub Vec<String>);

impl Friends {
    pub async fn get_by_id(
        user_id: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> ModelResult<Option<Self>> {
        // Fetch following and followers, limit to 10K
        let following = Following::get_by_id(user_id, None, Some(10000))
            .await?
            .unwrap_or_default()
            .0;

        let followers = Followers::get_by_id(user_id, None, Some(10000))
            .await?
            .unwrap_or_default()
            .0;

        // Find intersection of following and followers (mutual friends)
        let mut friends: Vec<String> = following
            .into_iter()
            .filter(|user_id| followers.contains(user_id))
            .collect();

        if friends.is_empty() {
            return Ok(None);
        }

        if let Some(skip) = skip {
            friends = friends.into_iter().skip(skip).collect();
        }
        if let Some(limit) = limit {
            friends.truncate(limit);
        }

        Ok(Some(Self(friends)))
    }

    // Checks whether user_a and user_b are friends
    pub async fn check(user_a_id: &str, user_b_id: &str) -> ModelResult<bool> {
        let user_a_key_parts = &[user_a_id][..];
        let user_b_key_parts = &[user_b_id][..];

        let ((_, a_follows_b), (_, b_follows_a)) = tokio::try_join!(
            Following::check_set_member(user_a_key_parts, user_b_id),
            Following::check_set_member(user_b_key_parts, user_a_id),
        )?;

        Ok(a_follows_b && b_follows_a)
    }
}
