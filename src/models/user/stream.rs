use super::{Muted, UserCounts, UserSearch, UserView};
use crate::models::follow::{Followers, Following, Friends, UserFollows};
use crate::types::DynError;
use crate::{db::kv::index::sorted_sets::SortOrder, RedisOps};
use serde::{Deserialize, Serialize};
use tokio::task::spawn;
use utoipa::ToSchema;

pub const USER_MOSTFOLLOWED_KEY_PARTS: [&str; 2] = ["Users", "MostFollowed"];
pub const USER_PIONEERS_KEY_PARTS: [&str; 2] = ["Users", "Pioneers"];

#[derive(Deserialize, ToSchema, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum UserStreamSource {
    Followers,
    Following,
    Friends,
    Muted,
    MostFollowed,
    Pioneers,
}

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct UserStream(Vec<UserView>);

impl RedisOps for UserStream {}

impl UserStream {
    pub async fn get_by_id(
        user_id: Option<&str>,
        viewer_id: Option<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
        source: UserStreamSource,
    ) -> Result<Option<Self>, DynError> {
        let user_ids = Self::get_user_list_from_source(user_id, source, skip, limit).await?;
        match user_ids {
            Some(users) => Self::from_listed_user_ids(&users, viewer_id).await,
            None => Ok(None),
        }
    }

    pub async fn get_from_username_search(
        username: &str,
        viewer_id: Option<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Self>, DynError> {
        let user_ids = UserSearch::get_by_name(username, skip, limit)
            .await?
            .map(|result| result.0);

        match user_ids {
            Some(users) => Self::from_listed_user_ids(&users, viewer_id).await,
            None => Ok(None),
        }
    }

    pub async fn from_listed_user_ids(
        user_ids: &[String],
        viewer_id: Option<&str>,
    ) -> Result<Option<Self>, DynError> {
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

        match user_views.is_empty() {
            true => Ok(None),
            false => Ok(Some(Self(user_views))),
        }
    }

    /// Adds the post to a Redis sorted set using the follower counts as score.
    pub async fn add_to_most_followed_sorted_set(
        user_id: &str,
        counts: &UserCounts,
    ) -> Result<(), DynError> {
        Self::put_index_sorted_set(
            &USER_MOSTFOLLOWED_KEY_PARTS,
            &[(counts.followers as f64, user_id)],
        )
        .await
    }

    /// Adds the post to a Redis sorted set using the follower counts as score.
    pub async fn add_to_pioneers_sorted_set(
        user_id: &str,
        counts: &UserCounts,
    ) -> Result<(), DynError> {
        let score = (counts.tags + counts.posts) as f64 * (counts.followers as f64).sqrt();
        Self::put_index_sorted_set(&USER_PIONEERS_KEY_PARTS, &[(score, user_id)]).await
    }

    // Get list of users based on the specified reach type
    pub async fn get_user_list_from_source(
        user_id: Option<&str>,
        source: UserStreamSource,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, DynError> {
        let user_ids = match source {
            UserStreamSource::Followers => Followers::get_by_id(
                user_id
                    .expect("User ID should be provided for user streams with source 'followers'"),
                skip,
                limit,
            )
            .await?
            .map(|u| u.0),
            UserStreamSource::Following => Following::get_by_id(
                user_id
                    .expect("User ID should be provided for user streams with source 'following'"),
                skip,
                limit,
            )
            .await?
            .map(|u| u.0),
            UserStreamSource::Friends => Friends::get_by_id(
                user_id.expect("User ID should be provided for user streams with source 'friends'"),
                skip,
                limit,
            )
            .await?
            .map(|u| u.0),
            UserStreamSource::Muted => Muted::get_by_id(
                user_id.expect("User ID should be provided for user streams with source 'muted'"),
                skip,
                limit,
            )
            .await?
            .map(|u| u.0),
            UserStreamSource::MostFollowed => Self::try_from_index_sorted_set(
                &USER_MOSTFOLLOWED_KEY_PARTS,
                None,
                None,
                skip,
                limit,
                SortOrder::Descending,
            )
            .await?
            .map(|set| set.into_iter().map(|(user_id, _score)| user_id).collect()),
            UserStreamSource::Pioneers => Self::try_from_index_sorted_set(
                &USER_PIONEERS_KEY_PARTS,
                None,
                None,
                skip,
                limit,
                SortOrder::Descending,
            )
            .await?
            .map(|set| set.into_iter().map(|(user_id, _score)| user_id).collect()),
        };
        Ok(user_ids)
    }
}
