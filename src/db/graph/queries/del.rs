use neo4rs::{query, Query};

// Delete a user node
// Will delete all relationships of this user as well!
pub fn delete_user(user_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $id})
         DETACH DELETE u;",
    )
    .param("id", user_id.to_string())
}

// Delete a post node
// Will delete all relationships of this user as well!
pub fn delete_post(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         DETACH DELETE p;",
    )
    .param("author_id", author_id.to_string())
    .param("post_id", post_id.to_string())
}

// Delete a follows relationship between two users
pub fn delete_follow(follower_id: &str, followee_id: &str) -> Query {
    query(
        "MATCH (follower:User {id: $follower_id})-[r:FOLLOWS]->(followee:User {id: $followee_id})
        
         DELETE r
         
         // returns whether the relationship existed as 'boolean'
         RETURN r IS NOT NULL AS boolean;",
    )
    .param("follower_id", follower_id.to_string())
    .param("followee_id", followee_id.to_string())
}

// Delete a muted relationship between two users
pub fn delete_mute(user_id: &str, muted_id: &str) -> Query {
    query(
        "MATCH (user:User {id: $user_id})-[r:MUTED]->(muted:User {id: $muted_id})
         DELETE r;",
    )
    .param("user_id", user_id.to_string())
    .param("muted_id", muted_id.to_string())
}

// Delete bookmarked relationship
pub fn delete_bookmark(user_id: &str, bookmark_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $user_id})-[b:BOOKMARKED {id: $bookmark_id}]->(post:Post)<-[:AUTHORED]-(author:User)
         WITH post.id as post_id, author.id as author_id, b
         DELETE b
         RETURN post_id, author_id",
    )
    .param("user_id", user_id)
    .param("bookmark_id", bookmark_id)
}

// Delete a tagged relationship
pub fn delete_tag(user_id: &str, tag_id: &str) -> Query {
    query(
        "MATCH (user:User {id: $user_id})-[t:TAGGED {id: $tag_id}]->(target)
         DELETE t",
    )
    .param("user_id", user_id)
    .param("tag_id", tag_id)
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
