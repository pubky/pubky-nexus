use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use neo4rs::Node;
use pkarr::PublicKey;
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use utoipa::ToSchema;

/// Represents a user's single link with a title and URL.
#[derive(Serialize, Deserialize, ToSchema, Default, Clone)]
pub struct UserLink {
    title: String,
    url: String,
}

impl UserLink {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            url: String::new(),
        }
    }
}

/// Represents user data with name, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Default, Clone)]
pub struct UserDetails {
    name: String,
    bio: String,
    id: String,
    links: Vec<UserLink>,
    status: String,
    indexed_at: i64,
}

impl UserDetails {
    /// Retrieves details by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        user_id: &str,
    ) -> Result<Option<UserDetails>, Box<dyn std::error::Error + Send + Sync>> {
        // Delegate to UserDetailsCollection::get_by_ids for single item retrieval
        let details_collection = UserDetailsCollection::get_by_ids(&[user_id]).await?;
        Ok(details_collection.0.into_iter().flatten().next())
    }

    async fn from_node(node: &Node) -> Option<Self> {
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
}

impl RedisOps for UserDetails {}

/// Represents a collection of UserDetails.
#[derive(Serialize, Deserialize)]
pub struct UserDetailsCollection(Vec<Option<UserDetails>>);

impl AsRef<[Option<UserDetails>]> for UserDetailsCollection {
    fn as_ref(&self) -> &[Option<UserDetails>] {
        &self.0
    }
}

// Implement Deref and DerefMut traits for UserDetailsCollection
impl Deref for UserDetailsCollection {
    type Target = Vec<Option<UserDetails>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for UserDetailsCollection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl RedisOps for UserDetailsCollection {}

impl UserDetailsCollection {
    /// Retrieves details for a list of user IDs, using Redis cache when available.
    pub async fn get_by_ids(
        user_ids: &[&str],
    ) -> Result<UserDetailsCollection, Box<dyn std::error::Error + Send + Sync>> {
        let key_parts_list: Vec<&[&str]> = user_ids.iter().map(std::slice::from_ref).collect();
        let mut user_details = UserDetailsCollection(
            UserDetails::try_from_index_multiple_json(&key_parts_list).await?,
        );

        let mut missing: Vec<(usize, &str)> = Vec::new();

        for (i, details) in user_details.iter().enumerate() {
            if details.is_none() {
                missing.push((i, user_ids[i]));
            }
        }

        if !missing.is_empty() {
            let missing_ids: Vec<&str> = missing.iter().map(|&(_, id)| id).collect();
            let fetched_details = Self::from_graph(&missing_ids).await?;

            for (i, (_, user_id)) in missing.iter().enumerate() {
                if let Some(index) = user_ids.iter().position(|&id| id == *user_id) {
                    user_details.0[index] = fetched_details.0[i].clone();
                }
            }
        }

        Ok(user_details)
    }

    /// Fetches user details from Neo4j and caches them in Redis.
    pub async fn from_graph(
        user_ids: &[&str],
    ) -> Result<UserDetailsCollection, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get_users_details_by_ids(user_ids);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;
        let mut user_details = UserDetailsCollection(Vec::new());

        while let Some(row) = result.next().await? {
            let node: Node = row.get("u")?;
            user_details.push(UserDetails::from_node(&node).await);
        }

        if !user_details.is_empty() {
            user_details.to_index(user_ids).await?;
        }

        Ok(user_details)
    }

    /// Indexes user details in Redis.
    pub async fn to_index(
        &self,
        user_ids: &[&str],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key_parts_list: Vec<Vec<&str>> = user_ids.iter().map(|id| vec![*id]).collect();
        let keys_refs: Vec<&[&str]> = key_parts_list.iter().map(|key| &key[..]).collect();
        self.put_multiple_json_indexes(&keys_refs).await
    }
}
