use pk_social_common::{
    connectors::neo4j::{Node, GLOBAL_NEO4J_CONNECTOR},
    queries,
};
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
        let graph = GLOBAL_NEO4J_CONNECTOR
            .get()
            .expect("Neo4jConnector not initialized")
            .graph
            .get()
            .expect("Not connected to Neo4j");

        let query = queries::get_user_by_id(user_id);

        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let node: Node = row.get("u").unwrap();
            Ok(Some(Self::from_neo4j_user_node(&node)))
        } else {
            Ok(None)
        }
    }
}
