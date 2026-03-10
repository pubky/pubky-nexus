use neo4rs::{query, Query};

/// Deletes a user node and all its relationships
/// # Arguments
/// * `user_id` - The unique identifier of the user to be deleted
pub fn delete_user(user_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $id})
         DETACH DELETE u;",
    )
    .param("id", user_id.to_string())
}

/// Deletes a post node authored by a specific user, along with all its relationships
/// # Arguments
/// * `author_id` - The unique identifier of the user who authored the post.
/// * `post_id` - The unique identifier of the post to be deleted.
pub fn delete_post(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         DETACH DELETE p;",
    )
    .param("author_id", author_id.to_string())
    .param("post_id", post_id.to_string())
}

/// Deletes a "follows" relationship between two users
/// # Arguments
/// * `follower_id` - The unique identifier of the user who is following another user.
/// * `followee_id` - The unique identifier of the user being followed
pub fn delete_follow(follower_id: &str, followee_id: &str) -> Query {
    query(
        "// Important that MATCH to check if both users are in the graph
        MATCH (follower:User {id: $follower_id}), (followee:User {id: $followee_id})
        // Check if follow already exist
        OPTIONAL MATCH (follower)-[existing:FOLLOWS]->(followee) 
        DELETE existing
        // Returns true if the relationship does not exist as 'flag'
        RETURN existing IS NULL AS flag;",
    )
    .param("follower_id", follower_id.to_string())
    .param("followee_id", followee_id.to_string())
}

/// Deletes a bookmark relationship between a user and a post
/// # Arguments
/// * `user_id` - The unique identifier of the user who created the bookmark.
/// * `bookmark_id` - The unique identifier of the bookmark relationship to be deleted.
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

/// Deletes a tag relationship created by a user and retrieves relevant details about the tag's target
/// # Arguments
/// * `user_id` - The unique identifier of the user who created the tag.
/// * `tag_id` - The unique identifier of the `TAGGED` relationship to be deleted.
pub fn delete_tag(user_id: &str, tag_id: &str) -> Query {
    query(
        "MATCH (user:User {id: $user_id})-[tag:TAGGED {id: $tag_id}]->(target)
         OPTIONAL MATCH (target)<-[:AUTHORED]-(author:User)
         WITH CASE WHEN target:User THEN target.id ELSE null END AS user_id,
              CASE WHEN target:Post THEN target.id ELSE null END AS post_id,
              CASE WHEN target:Post THEN author.id ELSE null END AS author_id,
              tag.label AS label,
              tag
         DELETE tag
         RETURN user_id, post_id, author_id, label",
    )
    .param("user_id", user_id)
    .param("tag_id", tag_id)
}

/// Deletes a file node and all its relationships
/// # Arguments
/// * `owner_id` - The unique identifier of the user who owns the file
/// * `file_id` - The unique identifier of the file to be deleted
pub fn delete_file(owner_id: &str, file_id: &str) -> Query {
    query(
        "MATCH (f:File {id: $id, owner_id: $owner_id})
         DETACH DELETE f;",
    )
    .param("id", file_id.to_string())
    .param("owner_id", owner_id.to_string())
}
