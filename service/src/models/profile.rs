use pk_social_common::connectors::neo4j::{Node, GLOBAL_NEO4J_CONNECTOR};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct Profile {
    pub bio: String,
    pub id: String,
    pub image: String,
    pub name: String,
    pub status: String,
}

impl Profile {
    pub fn from_neo4j_user_node(node: &Node) -> Self {
        Self {
            bio: node.get("bio").unwrap_or_default(),
            id: node.get("id").unwrap_or_default(),
            image: node.get("image").unwrap_or_default(),
            name: node.get("name").unwrap_or_default(),
            status: node.get("status").unwrap_or_default(),
        }
    }

    pub async fn get_by_id(user_id: &str) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let neo4j_connector = GLOBAL_NEO4J_CONNECTOR
            .get()
            .expect("Neo4jConnector not initialized");
        if let Some(node) = neo4j_connector.get_user_by_id(user_id).await? {
            Ok(Some(Self::from_neo4j_user_node(&node)))
        } else {
            Ok(None)
        }
    }
}
