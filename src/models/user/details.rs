use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use axum::async_trait;
use chrono::Utc;
use neo4rs::Node;
use pkarr::PublicKey;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct UserLink {
    title: String,
    url: String,
}

impl Default for UserLink {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a user's single link with a title and URL.
impl UserLink {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            url: String::new(),
        }
    }
}

/// Represents user data with name, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct UserDetails {
    name: String,
    bio: String,
    id: String,
    links: Vec<UserLink>,
    status: String,
    indexed_at: i64,
}

impl Default for UserDetails {
    fn default() -> Self {
        Self::new()
    }
}

impl RedisOps for UserDetails {}

#[derive(Serialize, Deserialize)]
pub struct UserDetailsCollection(Vec<UserDetails>);

impl AsRef<[UserDetails]> for UserDetailsCollection {
    fn as_ref(&self) -> &[UserDetails] {
        &self.0
    }
}

#[async_trait]
impl RedisOps for UserDetailsCollection {
    async fn prefix() -> String {
        String::from("UserDetails")
    }
}

impl UserDetails {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            bio: String::new(),
            id: String::new(),
            links: vec![UserLink::new()],
            status: String::new(),
            indexed_at: Utc::now().timestamp(),
        }
    }

    /// Retrieves details by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        user_id: &str,
    ) -> Result<Option<UserDetails>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::try_from_index_json(&[user_id]).await? {
            Some(details) => Ok(Some(details)),
            None => Self::get_from_graph(user_id).await,
        }
    }

    async fn from_node(node: &Node) -> Option<Self> {
        // TODO validation of ID should happen when we WRITE a Post into the graph with the watcher.
        let id: String = node.get("id").unwrap_or_default();
        PublicKey::try_from(id.clone()).ok()?;

        Some(Self {
            id,
            name: node.get("name").unwrap_or_default(),
            bio: node.get("bio").unwrap_or_default(),
            status: node.get("status").unwrap_or_default(),
            links: node.get("links").unwrap_or_default(),
            indexed_at: node.get("indexed_at").unwrap_or_default(),
        })
    }

    /// Retrieves the details from Neo4j.
    pub async fn get_from_graph(
        user_id: &str,
    ) -> Result<Option<UserDetails>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get_users_details_by_ids(&[user_id]);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        match result.next().await? {
            Some(row) => {
                let node: Node = row.get("u")?;
                match Self::from_node(&node).await {
                    Some(details) => {
                        details.put_index_json(&[user_id]).await?;
                        Ok(Some(details))
                    }
                    None => Ok(None),
                }
            }
            None => Ok(None),
        }
    }
}

impl UserDetailsCollection {
    /// Retrieves details for a list of user IDs, using Redis cache when available.
    pub async fn get_by_ids(
        user_ids: &[&str],
    ) -> Result<UserDetailsCollection, Box<dyn std::error::Error + Send + Sync>> {
        let cached_details = UserDetails::try_from_index_multiple_json(&[user_ids]).await?;

        let mut user_details: Vec<UserDetails> = Vec::new();
        let mut missing_ids = Vec::new();

        for (i, details) in cached_details.into_iter().enumerate() {
            match details {
                Some(detail) => user_details.push(detail),
                None => missing_ids.push(user_ids[i]),
            }
        }

        if !missing_ids.is_empty() {
            let fetched_details = Self::from_graph(&missing_ids).await?;
            user_details.extend(fetched_details);
        }

        Ok(UserDetailsCollection(user_details))
    }

    /// Fetches user details from Neo4j and caches them in Redis.
    pub async fn from_graph(
        user_ids: &[&str],
    ) -> Result<Vec<UserDetails>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get_users_details_by_ids(user_ids);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        let mut user_details = Vec::new();

        while let Some(row) = result.next().await? {
            let node: Node = row.get("u")?;
            if let Some(detail) = UserDetails::from_node(&node).await {
                user_details.push(detail);
            }
        }

        if !user_details.is_empty() {
            let key_parts_list: Vec<Vec<&str>> = user_details
                .iter()
                .map(|detail| vec![detail.id.as_str()])
                .collect();
            let keys_refs: Vec<&[&str]> = key_parts_list.iter().map(|key| &key[..]).collect();
            UserDetailsCollection(user_details.clone())
                .put_multiple_json_indexes(&keys_refs)
                .await?;
        }

        Ok(user_details)
    }
}
