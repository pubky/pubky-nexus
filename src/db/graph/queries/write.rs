use crate::models::user::UserDetails;
use neo4rs::{query, Query};

// Create a user node
pub fn create_user(user: &UserDetails) -> Query {
    let links = serde_json::to_string(&user.links).unwrap_or_else(|_| "[]".to_string());
    query(
        "MERGE (u:User {id: $id})
         SET u.name = $name, u.bio = $bio, u.status = $status, u.links = $links, u.indexed_at = $indexed_at;",
    )
    .param("id", user.id.to_string())
    .param("name", user.name.to_string())
    .param("bio", user.bio.as_ref().unwrap_or(&"null".to_string()).to_string())
    .param("status", user.status.as_ref().unwrap_or(&"null".to_string()).to_string())
    .param("links", links)
    .param("indexed_at", user.indexed_at)
}

// Delete a user node
// Will delete all relationships of this user as well!
pub fn delete_user(user_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $id})
         DETACH DELETE u;",
    )
    .param("id", user_id.to_string())
}
