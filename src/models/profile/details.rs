use crate::{
    db::connectors::{neo4j::get_neo4j_graph, redis::get_redis_conn},
    queries,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use neo4rs::Node;
use redis::AsyncCommands;

const PROFILE_DETAILS_PREFIX: &str = "profile-details!";

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileLink {
    title: String,
    url: String,
}

impl Default for ProfileLink {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a profile link with a title and URL.
impl ProfileLink {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            url: String::new(),
        }
    }
}

/// Represents profile data with name, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileDetails {
    name: String,
    bio: String,
    id: String,
    image: String,
    links: Vec<ProfileLink>,
    status: String,
}

impl Default for ProfileDetails {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileDetails {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            bio: String::new(),
            id: String::new(),
            image: String::new(),
            links: vec![ProfileLink::new()],
            status: String::new(),
        }
    }

    /// Retrieves details by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        user_id: &str,
    ) -> Result<Option<ProfileDetails>, Box<dyn std::error::Error>> {
        if let Some(details) = ProfileDetails::get_from_index(user_id).await? {
            return Ok(Some(details));
        }

        ProfileDetails::get_from_graph(user_id).await
    }

    fn from_node(node: &Node) -> Self {
        Self {
            name: node.get("name").unwrap_or_default(),
            bio: node.get("bio").unwrap_or_default(),
            id: node.get("id").unwrap_or_default(),
            image: node.get("image").unwrap_or_default(),
            status: node.get("status").unwrap_or_default(),
            links: vec![ProfileLink::new()],
        }
    }

    /// Sets the details in the Redis cache.
    pub async fn set_index(&self, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut redis_conn = get_redis_conn().await?;
        let cache_key = format!("{PROFILE_DETAILS_PREFIX}{user_id}");

        let details_json = serde_json::to_string(&self)?;

        redis_conn.set_ex(&cache_key, details_json, 3600).await?;
        Ok(())
    }

    /// Retrieves the details from the Redis cache.
    pub async fn get_from_index(
        user_id: &str,
    ) -> Result<Option<ProfileDetails>, Box<dyn std::error::Error>> {
        let mut redis_conn = get_redis_conn().await?;
        let cache_key = format!("{PROFILE_DETAILS_PREFIX}{user_id}");

        if let Ok(cached_details) = redis_conn.get::<_, String>(&cache_key).await {
            let details: ProfileDetails = serde_json::from_str(&cached_details)?;
            return Ok(Some(details));
        }

        Ok(None)
    }

    /// Retrieves the details from Neo4j.
    pub async fn get_from_graph(
        user_id: &str,
    ) -> Result<Option<ProfileDetails>, Box<dyn std::error::Error>> {
        let graph = get_neo4j_graph()?;
        let user_query = queries::get_user_by_id(user_id);

        let graph = graph.lock().await;
        let mut result = graph.execute(user_query).await?;

        if let Some(row) = result.next().await? {
            let node: Node = row.get("u").unwrap();
            let details = ProfileDetails::from_node(&node);
            details.set_index(user_id).await?;
            Ok(Some(details))
        } else {
            Ok(None)
        }
    }
}
