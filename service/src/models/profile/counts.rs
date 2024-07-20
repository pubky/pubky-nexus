use pk_social_common::{
    connectors::{
        neo4j::get_neo4j_graph,
        redis::{get_redis_conn, AsyncCommands},
    },
    queries,
};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const PROFILE_COUNTS_PREFIX: &str = "profile-counts!";

/// Represents total counts of relationships of a profile.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileCounts {
    pub tags: u32,
    pub posts: u32,
    pub following: u32,
    pub followers: u32,
    pub friends: u32,
}

impl Default for ProfileCounts {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileCounts {
    pub fn new() -> Self {
        Self {
            tags: 0,
            posts: 0,
            followers: 0,
            following: 0,
            friends: 0,
        }
    }

    /// Retrieves counts by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        user_id: &str,
    ) -> Result<Option<ProfileCounts>, Box<dyn std::error::Error>> {
        if let Some(counts) = ProfileCounts::get_from_index(user_id).await? {
            return Ok(Some(counts));
        }

        ProfileCounts::get_from_graph(user_id).await
    }

    /// Sets counts in the Redis cache.
    pub async fn set_index(&self, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut redis_conn = get_redis_conn().await?;
        let cache_key = format!("{PROFILE_COUNTS_PREFIX}{user_id}");

        let counts_json = serde_json::to_string(&self)?;

        redis_conn.set_ex(&cache_key, counts_json, 3600).await?;
        Ok(())
    }

    /// Retrieves the counts from the Redis cache.
    pub async fn get_from_index(
        user_id: &str,
    ) -> Result<Option<ProfileCounts>, Box<dyn std::error::Error>> {
        let mut redis_conn = get_redis_conn().await?;
        let cache_key = format!("{PROFILE_COUNTS_PREFIX}{user_id}");

        if let Ok(cached_counts) = redis_conn.get::<_, String>(&cache_key).await {
            let counts: ProfileCounts = serde_json::from_str(&cached_counts)?;
            return Ok(Some(counts));
        }

        Ok(None)
    }

    /// Retrieves the counts from Neo4j.
    pub async fn get_from_graph(
        user_id: &str,
    ) -> Result<Option<ProfileCounts>, Box<dyn std::error::Error>> {
        let graph = get_neo4j_graph()?;
        let counts_query = queries::profile_counts(user_id);

        let graph = graph.lock().await;
        let mut result = graph.execute(counts_query).await?;

        if let Some(row) = result.next().await? {
            if !row.get("user_exists").unwrap_or(false) {
                return Ok(None);
            }
            let counts = ProfileCounts {
                following: row.get("following_count").unwrap_or_default(),
                followers: row.get("followers_count").unwrap_or_default(),
                friends: row.get("friends_count").unwrap_or_default(),
                posts: row.get("posts_count").unwrap_or_default(),
                tags: row.get("tags_count").unwrap_or_default(),
            };
            counts.set_index(user_id).await?;
            Ok(Some(counts))
        } else {
            Ok(None)
        }
    }
}
