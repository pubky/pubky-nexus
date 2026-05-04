use crate::db::graph::Query;

/// Deletes a user node and all its relationships
/// # Arguments
/// * `user_id` - The unique identifier of the user to be deleted
pub fn delete_user(user_id: &str) -> Query {
    Query::new(
        "delete_user",
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
    Query::new(
        "delete_post",
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
    Query::new(
        "delete_follow",
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
    Query::new(
        "delete_bookmark",
        "MATCH (u:User {id: $user_id})-[b:BOOKMARKED {id: $bookmark_id}]->(post:Post)<-[:AUTHORED]-(author:User)
         WITH post.id as post_id, author.id as author_id, b
         DELETE b
         RETURN post_id, author_id",
    )
    .param("user_id", user_id)
    .param("bookmark_id", bookmark_id)
}

/// Deletes a tag relationship created by a user and retrieves relevant details about the tag's target.
///
/// When `app` is `Some`, adds `WHERE tag.app = $app` to scope deletion to a specific app namespace.
/// This prevents cross-app deletion for Resource tags where each app owns its own TAGGED relationship.
///
/// When `app` is `None`, adds `WHERE tag.app IS NULL` to only match relationships without an app
/// property. This prevents cross-app deletion when multiple apps tag the same URI with the same label.
pub fn delete_tag(user_id: &str, tag_id: &str, app: Option<&str>) -> Query {
    let app_filter = match app {
        Some(_) => "\n    WHERE tag.app = $app",
        None => "\n    WHERE tag.app IS NULL",
    };

    let cypher = format!(
        "MATCH (user:User {{id: $user_id}})-[tag:TAGGED {{id: $tag_id}}]->(target){app_filter}
    OPTIONAL MATCH (target)<-[:AUTHORED]-(author:User)
    WITH CASE WHEN target:User THEN target.id ELSE null END AS user_id,
         CASE WHEN target:Post THEN target.id ELSE null END AS post_id,
         CASE WHEN target:Post THEN author.id ELSE null END AS author_id,
         CASE WHEN target:Resource THEN target.id ELSE null END AS resource_id,
         tag.label AS label,
         tag.app AS app,
         tag, target
    DELETE tag
    WITH user_id, post_id, author_id, resource_id, label, app, target
    CALL {{
        WITH target, resource_id
        WITH target, resource_id WHERE resource_id IS NOT NULL
        AND NOT EXISTS {{ (target)<-[:TAGGED]-() }}
        DELETE target
    }}
    RETURN user_id, post_id, author_id, resource_id, label, app"
    );

    let mut query = Query::new("delete_tag", &cypher)
        .param("user_id", user_id)
        .param("tag_id", tag_id);

    if let Some(a) = app {
        query = query.param("app", a);
    }

    query
}

/// Removes the `HOSTED_BY` relationship from a user, if one exists.
pub fn remove_user_homeserver(user_id: &str) -> Query {
    Query::new(
        "remove_user_homeserver",
        "MATCH (u:User {id: $user_id})-[r:HOSTED_BY]->(:Homeserver)
         DELETE r;",
    )
    .param("user_id", user_id.to_string())
}

/// Deletes a file node and all its relationships
/// # Arguments
/// * `owner_id` - The unique identifier of the user who owns the file
/// * `file_id` - The unique identifier of the file to be deleted
pub fn delete_file(owner_id: &str, file_id: &str) -> Query {
    Query::new(
        "delete_file",
        "MATCH (f:File {id: $id, owner_id: $owner_id})
         DETACH DELETE f;",
    )
    .param("id", file_id.to_string())
    .param("owner_id", owner_id.to_string())
}
