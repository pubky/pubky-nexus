use super::{Influencers, Muted, UserCounts, UserSearch, UserView};
use crate::models::follow::{Followers, Following, Friends, UserFollows};
use crate::types::{DynError, StreamReach, Timeframe};
use std::collections::HashSet;

use crate::db::kv::SortOrder;
use crate::db::{get_neo4j_graph, queries, RedisOps};
use crate::models::post::{PostStream, POST_REPLIES_PER_POST_KEY_PARTS};
use serde::{Deserialize, Serialize};
use tokio::task::spawn;
use utoipa::ToSchema;

pub const USER_MOSTFOLLOWED_KEY_PARTS: [&str; 2] = ["Users", "MostFollowed"];
pub const USER_INFLUENCERS_KEY_PARTS: [&str; 2] = ["Users", "Influencers"];
pub const CACHE_USER_RECOMMENDED_KEY_PARTS: [&str; 3] = ["Cache", "Users", "Recommended"];
// TTL, 12HR
pub const CACHE_USER_RECOMMENDED_TTL: i64 = 12 * 60 * 60;

#[derive(Deserialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UserStreamSource {
    Followers,
    Following,
    Friends,
    Muted,
    MostFollowed,
    Influencers,
    Recommended,
    PostReplies,
}

pub struct UserStreamInput {
    pub user_id: Option<String>,
    pub skip: Option<usize>,
    pub limit: Option<usize>,
    pub source: UserStreamSource,
    pub reach: Option<StreamReach>,
    pub timeframe: Option<Timeframe>,
    pub preview: Option<bool>,
    pub author_id: Option<String>,
    pub post_id: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct UserStream(Vec<UserView>);

impl RedisOps for UserStream {}

impl UserStream {
    pub async fn get_by_id(
        input: UserStreamInput,
        viewer_id: Option<String>,
        depth: Option<u8>,
    ) -> Result<Option<Self>, DynError> {
        let user_ids = Self::get_user_list_from_source(input).await?;
        match user_ids {
            Some(users) => Self::from_listed_user_ids(&users, viewer_id.as_deref(), depth).await,
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
            Some(users) => Self::from_listed_user_ids(&users, viewer_id, None).await,
            None => Ok(None),
        }
    }

    pub async fn from_listed_user_ids(
        user_ids: &[String],
        viewer_id: Option<&str>,
        depth: Option<u8>,
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
                spawn(
                    async move { UserView::get_by_id(&user_id, viewer_id.as_deref(), depth).await },
                );
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
            None,
            None,
        )
        .await
    }

    /// Adds the post to a Redis sorted set using the follower counts as score.
    pub async fn add_to_influencers_sorted_set(
        user_id: &str,
        counts: &UserCounts,
    ) -> Result<(), DynError> {
        let score = (counts.tagged + counts.posts) as f64 * (counts.followers as f64).sqrt();
        Self::put_index_sorted_set(&USER_INFLUENCERS_KEY_PARTS, &[(score, user_id)], None, None)
            .await
    }
    /// Retrieves recommended user IDs based on the specified criteria.
    async fn get_recommended_ids(
        user_id: &str,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, DynError> {
        let count = limit.unwrap_or(5) as isize;

        // Attempt to get cached data from Redis
        if let Some(cached_data) = Self::try_get_cached_recommended(user_id, count).await? {
            return Ok(Some(cached_data));
        }

        // Cache miss; proceed to query Neo4j
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            // Query Neo4j for 30 user IDs
            let query = queries::get::recommend_users(user_id, 30);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        let mut user_ids = Vec::new();

        while let Some(row) = result.next().await? {
            if let Some(user_id) = row.get::<Option<String>>("recommended_user_id")? {
                user_ids.push(user_id);
            }
        }

        if user_ids.is_empty() {
            Ok(None)
        } else {
            Self::cache_recommended_users(user_id, &user_ids).await?;
            if let Some(limit) = limit {
                user_ids.truncate(limit);
            };
            Ok(Some(user_ids))
        }
    }

    async fn try_get_cached_recommended(
        user_id: &str,
        count: isize,
    ) -> Result<Option<Vec<String>>, DynError> {
        let key_parts = &["Cache", "Recommended", user_id];
        Self::try_get_random_from_index_set(
            key_parts,
            count,
            Some(CACHE_USER_RECOMMENDED_KEY_PARTS.join(":")),
        )
        .await
    }

    /// Helper method to cache recommended users in Redis with a TTL.
    async fn cache_recommended_users(user_id: &str, user_ids: &[String]) -> Result<(), DynError> {
        let values: Vec<&str> = user_ids.iter().map(|s| s.as_str()).collect();
        // Cache the result in Redis with a TTL of 12 hours
        Self::put_index_set(
            &[user_id],
            &values,
            Some(CACHE_USER_RECOMMENDED_TTL),
            Some(CACHE_USER_RECOMMENDED_KEY_PARTS.join(":")),
        )
        .await
    }

    async fn get_post_replies_ids(
        post_id: Option<String>,
        author_id: Option<String>,
    ) -> Result<Option<Vec<String>>, DynError> {
        let post_id = post_id
            .ok_or("Post ID should be provided for user streams with source 'post_replies'")?;
        let author_id = author_id
            .ok_or("Author ID should be provided for user streams with source 'post_replies'")?;
        let key_parts = [
            &POST_REPLIES_PER_POST_KEY_PARTS[..],
            &[author_id.as_str(), post_id.as_str()],
        ]
        .concat();
        let replies = PostStream::try_from_index_sorted_set(
            &key_parts,
            None,
            None,
            None,
            None,
            SortOrder::Descending,
            None,
        )
        .await?;

        // If there are replies, extract unique user IDs using a HashSet.
        let unique_user_ids: HashSet<String> = if let Some(replies) = replies {
            replies
                .into_iter()
                .filter_map(|reply| reply.0.split(':').next().map(|s| s.to_string()))
                .collect()
        } else {
            // If no replies are found, return None.
            return Ok(None);
        };

        // Convert the HashSet to a Vec. (Note: the ordering will be arbitrary.)
        Ok(Some(unique_user_ids.into_iter().collect()))
    }

    // Get list of users based on the specified reach type
    pub async fn get_user_list_from_source(
        input: UserStreamInput,
    ) -> Result<Option<Vec<String>>, DynError> {
        let UserStreamInput {
            user_id,
            skip,
            limit,
            source,
            reach,
            timeframe,
            preview,
            author_id,
            post_id,
        } = input;
        let user_ids =
            match source {
                UserStreamSource::Followers => Followers::get_by_id(
                    user_id
                        .ok_or(
                            "User ID should be provided for user streams with source 'followers'"
                                .to_string(),
                        )?
                        .as_str(),
                    skip,
                    limit,
                )
                .await?
                .map(|u| u.0),
                UserStreamSource::Following => Following::get_by_id(
                    user_id
                        .ok_or(
                            "User ID should be provided for user streams with source 'following'"
                                .to_string(),
                        )?
                        .as_str(),
                    skip,
                    limit,
                )
                .await?
                .map(|u| u.0),
                UserStreamSource::Friends => Friends::get_by_id(
                    user_id
                        .ok_or(
                            "User ID should be provided for user streams with source 'friends'"
                                .to_string(),
                        )?
                        .as_str(),
                    skip,
                    limit,
                )
                .await?
                .map(|u| u.0),
                UserStreamSource::Muted => Muted::get_by_id(
                    user_id
                        .ok_or(
                            "User ID should be provided for user streams with source 'muted'"
                                .to_string(),
                        )?
                        .as_str(),
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
                    None,
                )
                .await?
                .map(|set| set.into_iter().map(|(user_id, _score)| user_id).collect()),
                UserStreamSource::Influencers => Influencers::get_influencers(
                    user_id.as_deref(),
                    Some(reach.unwrap_or(StreamReach::Wot(3))),
                    skip.unwrap_or(0),
                    limit.unwrap_or(10).min(100),
                    timeframe.unwrap_or(Timeframe::AllTime),
                    preview.unwrap_or(false),
                )
                .await?
                .map(|result| {
                    result
                        .iter()
                        .map(|(influencer_id, _)| influencer_id.clone())
                        .collect()
                }),
                UserStreamSource::Recommended => UserStream::get_recommended_ids(
                    user_id
                        .ok_or(
                            "User ID should be provided for user streams with source 'recommended'"
                                .to_string(),
                        )?
                        .as_str(),
                    limit,
                )
                .await?,
                UserStreamSource::PostReplies => {
                    UserStream::get_post_replies_ids(post_id, author_id).await?
                }
            };
        Ok(user_ids)
    }
}
