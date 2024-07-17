use pk_social_common::connectors::neo4j::Node;
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
        // Correct type reference
        Self {
            bio: node.get("bio").unwrap_or_default(),
            id: node.get("id").unwrap_or_default(),
            image: node.get("image").unwrap_or_default(),
            name: node.get("name").unwrap_or_default(),
            status: node.get("status").unwrap_or_default(),
        }
    }
}
