use crate::models::{post::PostDetails, user::UserDetails};
use neo4rs::{query, Query};

// Create a user node
pub fn create_user(user: &UserDetails) -> Result<Query, Box<dyn std::error::Error + Send + Sync>> {
    let links = serde_json::to_string(&user.links)?;

    let query = query(
        "MERGE (u:User {id: $id})
         SET u.name = $name, u.bio = $bio, u.status = $status, u.links = $links, u.indexed_at = $indexed_at;",
    )
    .param("id", user.id.as_ref())
    .param("name", user.name.to_string())
    .param("bio", user.bio.as_ref().unwrap_or(&"null".to_string()).to_string())
    .param("status", user.status.as_ref().unwrap_or(&"null".to_string()).to_string())
    .param("links", links)
    .param("indexed_at", user.indexed_at);

    Ok(query)
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

// Create a post node
pub fn create_post(post: &PostDetails) -> Result<Query, Box<dyn std::error::Error + Send + Sync>> {
    let query = query(
        "MATCH (u:User {id: $author_id})
         MERGE (u)-[:AUTHORED]->(p:Post {id: $post_id})
         SET p.content = $content,
             p.indexed_at = $indexed_at,
             p.kind = $kind;",
    )
    .param("author_id", post.author.to_string())
    .param("post_id", post.id.to_string())
    .param("content", post.content.to_string())
    .param("indexed_at", post.indexed_at)
    .param("kind", post.kind.to_string());

    Ok(query)
}

/// Create a follows relationship between two users
/// Validates that both users exist before creating the relationship
pub fn create_follow(follower_id: &str, followee_id: &str, indexed_at: i64) -> Query {
    query(
        "MATCH (follower:User {id: $follower_id}), (followee:User {id: $followee_id})
         MERGE (follower)-[:FOLLOWS {indexed_at: $indexed_at}]->(followee);",
    )
    .param("follower_id", follower_id.to_string())
    .param("followee_id", followee_id.to_string())
    .param("indexed_at", indexed_at)
}

/// Delete a follows relationship between two users
pub fn delete_follow(follower_id: &str, followee_id: &str) -> Query {
    query(
        "MATCH (follower:User {id: $follower_id})-[r:FOLLOWS]->(followee:User {id: $followee_id})
         DELETE r;",
    )
    .param("follower_id", follower_id.to_string())
    .param("followee_id", followee_id.to_string())
}

pub fn create_post_bookmark(
    user_id: &str,
    author_id: &str,
    post_id: &str,
    bookmark_id: &str,
    indexed_at: i64,
) -> Query {
    query(
        "MATCH (u:User {id: $user_id})
         MATCH (author:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         MERGE (u)-[b:BOOKMARKED {id: $bookmark_id, indexed_at: $indexed_at}]->(p)",
    )
    .param("user_id", user_id)
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("bookmark_id", bookmark_id)
    .param("indexed_at", indexed_at)
}

pub fn delete_bookmark(user_id: &str, bookmark_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $user_id})-[b:BOOKMARKED {id: $bookmark_id}]->(target)
         DELETE b",
    )
    .param("user_id", user_id)
    .param("bookmark_id", bookmark_id)
}
