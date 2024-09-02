use crate::models::{file::FileDetails, user::UserDetails};
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

// Create a file node
pub fn create_file(file: &FileDetails) -> Query {
    let urls = serde_json::to_string(&file.urls).unwrap_or_else(|_| "{}".to_string());
    query(
        "MERGE (f:File {id: $id, owner_id: $owner_id})
         SET f.uri = $uri, f.indexed_at = $indexed_at, f.created_at = $created_at, f.size = $size,
            f.src = $src, f.content_type = $content_type, f.urls = $urls;",
    )
    .param("id", file.id.to_string())
    .param("owner_id", file.owner_id.to_string())
    .param("uri", file.uri.to_string())
    .param("indexed_at", file.indexed_at)
    .param("created_at", file.created_at)
    .param("size", file.size.to_string())
    .param("src", file.src.to_string())
    .param("name", file.name.to_string())
    .param("content_type", file.content_type.to_string())
    .param("urls", urls)
}

// Delete a file node
pub fn delete_file(owner_id: &str, file_id: &str) -> Query {
    query(
        "MATCH (f:File {id: $id, owner_id: $owner_id})
         DETACH DELETE f;",
    )
    .param("id", file_id.to_string())
    .param("owner_id", owner_id.to_string())
}
