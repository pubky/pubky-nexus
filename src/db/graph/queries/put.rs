use crate::events::uri::ParsedUri;
use crate::models::post::PostRelationships;
use crate::models::{file::FileDetails, post::PostDetails, user::UserDetails};
use crate::types::DynError;
use neo4rs::{query, Query};

// Create a user node
pub fn create_user(user: &UserDetails) -> Result<Query, DynError> {
    let links = serde_json::to_string(&user.links)?;

    let query = query(
        "MERGE (u:User {id: $id})
         SET u.name = $name, u.bio = $bio, u.status = $status, u.links = $links, u.image = $image, u.indexed_at = $indexed_at;",
    )
    .param("id", user.id.to_string())
    .param("name", user.name.clone())
    .param("bio", user.bio.clone())
    .param("status", user.status.clone())
    .param("links", links)
    .param("image", user.image.clone())
    .param("indexed_at", user.indexed_at);

    Ok(query)
}

/// Creates a Cypher query to add or edit a post to the graph database and handles its relationships.
/// # Arguments
/// * `post` - A reference to a `PostDetails` struct containing information about the post to be created or edited
/// * `post_relationships` - A reference to a PostRelationships struct that define relationships
///   for the post (e.g., replies or reposts).
pub fn create_post(
    post: &PostDetails,
    post_relationships: &PostRelationships,
) -> Result<Query, DynError> {
    let mut cypher = String::new();
    let mut new_relationships = Vec::new();

    // Check if all the dependencies are consistent in the graph
    if post_relationships.replied.is_some() {
        cypher.push_str("
            MATCH (reply_parent_author:User {id: $reply_parent_author_id})-[:AUTHORED]->(reply_parent_post:Post {id: $reply_parent_post_id})
        ");
        new_relationships.push("MERGE (new_post)-[:REPLIED]->(reply_parent_post)");
    };
    if post_relationships.reposted.is_some() {
        cypher.push_str("
            MATCH (repost_parent_author:User {id: $repost_parent_author_id})-[:AUTHORED]->(repost_parent_post:Post {id: $repost_parent_post_id})
        ");
        new_relationships.push("MERGE (new_post)-[:REPOSTED]->(repost_parent_post)");
    }
    // Create the new post
    cypher.push_str(
        "
        MATCH (author:User {id: $author_id})
        OPTIONAL MATCH (u)-[:AUTHORED]->(existing_post:Post {id: $post_id})
        MERGE (author)-[:AUTHORED]->(new_post:Post {id: $post_id})
    ",
    );

    // Add relationships to the query
    cypher.push_str(&new_relationships.join("\n"));

    cypher.push_str(
        "
        SET new_post.content = $content,
            new_post.indexed_at = $indexed_at,
            new_post.kind = $kind,
            new_post.attachments = $attachments
        RETURN existing_post IS NOT NULL AS boolean",
    );

    let kind = serde_json::to_string(&post.kind)?;

    let mut cypher_query = query(&cypher)
        .param("author_id", post.author.to_string())
        .param("post_id", post.id.to_string())
        .param("content", post.content.to_string())
        .param("indexed_at", post.indexed_at)
        .param("kind", kind.trim_matches('"'))
        .param(
            "attachments",
            post.attachments.clone().unwrap_or(vec![] as Vec<String>),
        );

    // Handle "replied" relationship
    cypher_query = add_relationship_params(
        cypher_query,
        &post_relationships.replied,
        "reply_parent_author_id",
        "reply_parent_post_id",
    )?;

    // Handle "reposted" relationship
    cypher_query = add_relationship_params(
        cypher_query,
        &post_relationships.reposted,
        "repost_parent_author_id",
        "repost_parent_post_id",
    )?;

    Ok(cypher_query)
}

fn add_relationship_params(
    cypher_query: Query,
    uri: &Option<String>,
    author_param: &str,
    post_param: &str,
) -> Result<Query, DynError> {
    if let Some(uri) = uri {
        let parsed_uri = ParsedUri::try_from(uri.as_str())?;
        let parent_author_id = parsed_uri.user_id;
        let parent_post_id = parsed_uri.post_id.ok_or("Missing post ID")?;

        return Ok(cypher_query
            .param(author_param, parent_author_id.as_str())
            .param(post_param, parent_post_id.as_str()));
    }
    Ok(cypher_query)
}

// Create a mentioned relationship between a post and a user
pub fn create_mention_relationship(
    author_id: &str,
    post_id: &str,
    mentioned_user_id: &str,
) -> Query {
    query(
        "MATCH (author:User {id: $author_id})-[:AUTHORED]->(post:Post {id: $post_id}),
              (mentioned_user:User {id: $mentioned_user_id})
         MERGE (post)-[:MENTIONED]->(mentioned_user)",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("mentioned_user_id", mentioned_user_id)
}

/// Create a follows relationship between two users
/// Validates that both users exist before creating the relationship
pub fn create_follow(follower_id: &str, followee_id: &str, indexed_at: i64) -> Query {
    query(
        "MATCH (follower:User {id: $follower_id}), (followee:User {id: $followee_id})

         // Check if follow already existed
         OPTIONAL MATCH (follower)-[existing:FOLLOWS]->(followee) 

         // Write data
         MERGE (follower)-[r:FOLLOWS]->(followee)
         SET r.indexed_at = $indexed_at

         // boolean == existed
         RETURN existing IS NOT NULL AS flag;",
    )
    .param("follower_id", follower_id.to_string())
    .param("followee_id", followee_id.to_string())
    .param("indexed_at", indexed_at)
}

/// Create a muted relationship between two users
pub fn create_mute(user_id: &str, muted_id: &str, indexed_at: i64) -> Query {
    query(
        "MATCH (user:User {id: $user_id}), (muted:User {id: $muted_id})
        // Check if follow already existed
        OPTIONAL MATCH (user)-[existing:MUTED]->(muted) 

        MERGE (user)-[r:MUTED]->(muted)
        SET r.indexed_at = $indexed_at

        // boolean == existed
        RETURN existing IS NOT NULL AS flag;",
    )
    .param("user_id", user_id.to_string())
    .param("muted_id", muted_id.to_string())
    .param("indexed_at", indexed_at)
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
        // We assume these nodes are already created. If not we would not be able to add a bookmark
         MATCH (author:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})

         // Check if bookmark already existed
         OPTIONAL MATCH (u)-[existing:BOOKMARKED]->(p) 

         // Write data
         MERGE (u)-[b:BOOKMARKED]->(p)
         SET b.indexed_at = $indexed_at,
             b.id = $bookmark_id
         
         // flag == existed
         RETURN existing IS NOT NULL AS flag;",
    )
    .param("user_id", user_id)
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("bookmark_id", bookmark_id)
    .param("indexed_at", indexed_at)
}

pub fn create_post_tag(
    user_id: &str,
    author_id: &str,
    post_id: &str,
    tag_id: &str,
    label: &str,
    indexed_at: i64,
) -> Query {
    query(
        "MATCH (user:User {id: $user_id})
        // We assume these nodes are already created. If not we would not be able to add a tag
        MATCH (author:User {id: $author_id})-[:AUTHORED]->(post:Post {id: $post_id})

         // Check if tag already existed
         OPTIONAL MATCH (user)-[existing:TAGGED {label: $label}]->(post) 

         // Write data
         MERGE (user)-[t:TAGGED {label: $label}]->(post)
         SET t.indexed_at = $indexed_at,
             t.id = $tag_id

         RETURN existing IS NOT NULL AS flag;",
    )
    .param("user_id", user_id)
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("tag_id", tag_id)
    .param("label", label)
    .param("indexed_at", indexed_at)
}

pub fn create_user_tag(
    tagger_user_id: &str,
    tagged_user_id: &str,
    tag_id: &str,
    label: &str,
    indexed_at: i64,
) -> Query {
    query(
        "MATCH (tagged_used:User {id: $tagged_user_id})
         MATCH (tagger:User {id: $tagger_user_id})

         // Check if tag already existed
         OPTIONAL MATCH (tagger)-[existing:TAGGED {label: $label}]->(tagged_used) 

         // Write data
         MERGE (tagger)-[t:TAGGED {label: $label}]->(tagged_used)
         SET t.indexed_at = $indexed_at,
             t.id = $tag_id

         // boolean == existed
         RETURN existing IS NOT NULL AS boolean;",
    )
    .param("tagger_user_id", tagger_user_id)
    .param("tagged_user_id", tagged_user_id)
    .param("tag_id", tag_id)
    .param("label", label)
    .param("indexed_at", indexed_at)
}

// Create a file node
pub fn create_file(file: &FileDetails) -> Result<Query, DynError> {
    let urls = serde_json::to_string(&file.urls)?;

    let query = query(
        "MERGE (f:File {id: $id, owner_id: $owner_id})
         SET f.uri = $uri, f.indexed_at = $indexed_at, f.created_at = $created_at, f.size = $size,
            f.src = $src, f.name = $name, f.content_type = $content_type, f.urls = $urls;",
    )
    .param("id", file.id.to_string())
    .param("owner_id", file.owner_id.to_string())
    .param("uri", file.uri.to_string())
    .param("indexed_at", file.indexed_at)
    .param("created_at", file.created_at)
    .param("size", file.size)
    .param("src", file.src.to_string())
    .param("name", file.name.to_string())
    .param("content_type", file.content_type.to_string())
    .param("urls", urls);

    Ok(query)
}
