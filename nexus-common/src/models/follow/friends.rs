use crate::db::RedisOps;
use crate::models::error::ModelResult;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
        let following_opt = Following::get_by_id(user_id, None, Some(10000)).await?;
        let followers_opt = Followers::get_by_id(user_id, None, Some(10000)).await?;

        // A `Some` (even if empty) from either side means the user exists; `None`
        // from both means the user was not found. This lets an existing user with
        // no mutual friends return `Some(empty)` (200 []) instead of a 404.
        let user_exists = following_opt.is_some() || followers_opt.is_some();

        let following = following_opt.unwrap_or_default().0;
        let followers: HashSet<String> = followers_opt.unwrap_or_default().0.into_iter().collect();

        // Intersection of following and followers (mutual friends), O(n + m).
        let mut friends: Vec<String> = following
            .into_iter()
            .filter(|user_id| followers.contains(user_id))
            .collect();

        if friends.is_empty() && !user_exists {
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
