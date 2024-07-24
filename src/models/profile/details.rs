use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, RedisOps};
use chrono::Utc;
use neo4rs::Node;
use pkarr::PublicKey;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
    links: Vec<ProfileLink>,
    status: String,
    indexed_at: i64,
}

impl Default for ProfileDetails {
    fn default() -> Self {
        Self::new()
    }
}

impl RedisOps for ProfileDetails {}

impl ProfileDetails {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            bio: String::new(),
            id: String::new(),
            links: vec![ProfileLink::new()],
            status: String::new(),
            indexed_at: Utc::now().timestamp(),
        }
    }

    /// Retrieves details by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        user_id: &str,
    ) -> Result<Option<ProfileDetails>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::try_from_index(user_id).await? {
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
    ) -> Result<Option<ProfileDetails>, Box<dyn std::error::Error + Send + Sync>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get_user_by_id(user_id);

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        match result.next().await? {
            Some(row) => {
                let node: Node = row.get("u")?;
                match Self::from_node(&node).await {
                    Some(details) => {
                        details.set_index(user_id).await?;
                        Ok(Some(details))
                    }
                    None => Ok(None),
                }
            }
            None => Ok(None),
        }
    }
}
