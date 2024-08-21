use crate::db::connectors::neo4j::get_neo4j_graph;
use neo4rs::{query, Query};

// Set graph constraints if they do not already exist
pub async fn setup_graph() -> Result<(), Box<dyn std::error::Error>> {
    let constraints = [
        "CREATE CONSTRAINT uniqueUserId IF NOT EXISTS FOR (u:User) REQUIRE u.id IS UNIQUE",
        "CREATE CONSTRAINT uniquePostId IF NOT EXISTS FOR (p:Post) REQUIRE p.id IS UNIQUE",
    ];

    let indexes = [
        "CREATE INDEX userIdIndex IF NOT EXISTS FOR (u:User) ON (u.id)",
        "CREATE INDEX postIdIndex IF NOT EXISTS FOR (p:Post) ON (p.id)",
        "CREATE INDEX taggedLabelIndex IF NOT EXISTS FOR ()-[r:TAGGED]-() ON (r.label)",
    ];

    let queries = constraints.iter().chain(indexes.iter());

    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;
    for q in queries {
        graph.run(query(q)).await?;
    }

    Ok(())
}

// Create nodes with Merge (avoid key duplication). Examples:
// MERGE (u:User {id: "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"}) SET u.name = "Aldert", u.status = "working", u.links = ...
// MERGE (p:Post {id: "0RDV7ABDZDW0"}) SET p.content = "Julian Assange is free", p.uri = "pubky:pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pubky.app/posts/0RDV7ABDZDW0", p.createdAt = 1719308315917;

// Retrieve post node by post id and author id
pub fn get_post_by_id(author_id: &str, post_id: &str) -> Query {
    query("MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id}) RETURN p")
        .param("author_id", author_id)
        .param("post_id", post_id)
}

pub fn post_counts(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         OPTIONAL MATCH (p)<-[tag:TAGGED]-()
         OPTIONAL MATCH (p)<-[reply:REPLIED]-()
         OPTIONAL MATCH (p)<-[repost:REPOSTED]-()
         RETURN COUNT(p) > 0 AS post_exists,
                COUNT(DISTINCT tag) AS tags_count,
                COUNT(DISTINCT reply) AS replies_count,
                COUNT(DISTINCT repost) AS reposts_count",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

pub fn post_bookmark(author_id: &str, post_id: &str, viewer_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         OPTIONAL MATCH (viewer:User {id: $viewer_id})-[b:BOOKMARKED]->(p)
         RETURN b",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("viewer_id", viewer_id)
}

pub fn user_bookmarks(user_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $user_id})-[b:BOOKMARKED]->(p:Post)<-[:AUTHORED]-(author:User)
         RETURN b, p.id AS post_id, author.id AS author_id",
    )
    .param("user_id", user_id)
}

pub fn post_relationships(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        OPTIONAL MATCH (p)-[:REPLIED]->(replied_post:Post)<-[:AUTHORED]-(replied_author:User)
        OPTIONAL MATCH (p)-[:REPOSTED]->(reposted_post:Post)<-[:AUTHORED]-(reposted_author:User)
        OPTIONAL MATCH (p)-[:MENTIONED]->(mentioned_user:User)
        RETURN 
          replied_post.id AS replied_post_id, 
          replied_author.id AS replied_author_id,
          reposted_post.id AS reposted_post_id, 
          reposted_author.id AS reposted_author_id,
          COLLECT(mentioned_user.id) AS mentioned_user_ids",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

// Retrieve many users by id
// We return also id if not we will not get not found users
pub fn get_users_details_by_ids(user_ids: &[&str]) -> Query {
    query(
        "
        UNWIND $ids AS id
        OPTIONAL MATCH (user:User {id: id})
        RETURN 
            id,
            CASE 
                WHEN user IS NOT NULL 
                    THEN user { .id, .bio, .status, .name, .indexed_at, .links }
                    ELSE null
                END AS record
        ",
    )
    .param("ids", user_ids)
}

/// Retrieve all the tags of the post
pub fn post_tags(user_id: &str, post_id: &str) -> neo4rs::Query {
    query(
        "
        MATCH (u:User {id: $user_id})-[:AUTHORED]->(p:Post {id: $post_id})
        CALL {
            WITH p
            MATCH (tagger:User)-[tag:TAGGED]->(p)
            WITH tag.label AS name,
                collect({
                    tag_id: tag.id,
                    indexed_at: tag.indexed_at,
                    tagger_id: tagger.id
                }) AS from
            RETURN collect({
                label: name,
                tagged: from
            }) AS post_tags
        }
        RETURN 
            u IS NOT NULL AS post_exists,
            post_tags
    ",
    )
    .param("user_id", user_id)
    .param("post_id", post_id)
}

pub fn user_tags(user_id: &str) -> neo4rs::Query {
    query(
        "
        MATCH (u:User {id: $user_id})
        CALL {
            WITH u
            MATCH (p:User)-[r:TAGGED]->(u)
            WITH r.label AS name,
                collect({
                    tag_id: r.id,
                    indexed_at: r.indexed_at,
                    tagger_id: p.id
                }) AS from
            RETURN collect({
                label: name,
                tagged: from
            }) AS user_tags
        }
        RETURN 
            u IS NOT NULL AS user_exists,
            user_tags
    ",
    )
    .param("user_id", user_id)
}

pub fn user_counts(user_id: &str) -> neo4rs::Query {
    query(
        "MATCH (u:User {id: $id})
           OPTIONAL MATCH (u)-[:FOLLOWS]->(following:User)
           OPTIONAL MATCH (follower:User)-[:FOLLOWS]->(u)
           OPTIONAL MATCH (u)-[:FOLLOWS]->(friend:User)-[:FOLLOWS]->(u)
           OPTIONAL MATCH (u)-[:AUTHORED]->(post:Post)
           OPTIONAL MATCH (u)-[tag:TAGGED]->(:Post)
           RETURN COUNT(u) > 0 AS user_exists,
                  COUNT(DISTINCT following) AS following_count,
                  COUNT(DISTINCT follower) AS followers_count,
                  COUNT(DISTINCT friend) AS friends_count,
                  COUNT(DISTINCT post) AS posts_count,
                  COUNT(DISTINCT tag) AS tags_count",
    )
    .param("id", user_id)
}

pub fn get_user_followers(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
    let mut query_string = String::from(
        "MATCH (u:User {id: $user_id}) 
         OPTIONAL MATCH (u)<-[:FOLLOWS]-(follower:User)
         RETURN COUNT(u) > 0 AS user_exists, 
                COLLECT(follower.id) AS follower_ids",
    );
    if let Some(skip_value) = skip {
        query_string.push_str(&format!(" SKIP {}", skip_value));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {}", limit_value));
    }
    query(&query_string).param("user_id", user_id)
}

pub fn get_user_following(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
    let mut query_string = String::from(
        "MATCH (u:User {id: $user_id}) 
         OPTIONAL MATCH (u)-[:FOLLOWS]->(following:User)
         RETURN COUNT(u) > 0 AS user_exists, 
                COLLECT(following.id) AS following_ids",
    );
    if let Some(skip_value) = skip {
        query_string.push_str(&format!(" SKIP {}", skip_value));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {}", limit_value));
    }
    query(&query_string).param("user_id", user_id)
}

// Retrieves popular tags across the entire network
// Results ordered by post count (descending), effectively ranking "hot" tags.
pub fn get_global_hot_tags_scores(threshold: u64) -> Query {
    query(
        "
        MATCH (u:User)-[tag:TAGGED]->(p:Post)
        WITH tag.label AS label, COUNT(DISTINCT p) AS uniquePosts
        WHERE uniquePosts >= $threshold
        RETURN COLLECT([toFloat(uniquePosts), label]) AS hot_tags
    ",
    )
    .param("threshold", threshold as f64)
}

// Retrieves popular hot tags taggers across the entire network
pub fn get_global_hot_tags_taggers(tag_list: &[&str]) -> Query {
    query(
        "
        UNWIND $labels AS tag_name
        MATCH (u:User)-[tag:TAGGED]->(p:Post)
        WHERE tag.label = tag_name
        WITH tag.label AS label, COLLECT(DISTINCT u.id) AS userIds
        RETURN COLLECT(userIds) AS tag_user_ids
    ",
    )
    .param("labels", tag_list)
}

// Analyzes tag usage for a specific list of user IDs. Groups tags by name,
// showing for each: label, post count and list of user IDs
// Orders by post_count (descending).
// Note: Only considers users from the provided users_id list.
pub fn get_tags_by_user_ids(users_id: &[&str]) -> Query {
    query(
        "
        UNWIND $ids AS id
        MATCH (u:User)-[tag:TAGGED]->(p:Post)
        WHERE u.id = id
        WITH tag.label AS label, COLLECT(DISTINCT u.id) AS taggers, COUNT(DISTINCT p) AS uniquePosts
        WITH {
            label: label,
            tagger_ids: taggers,
            post_count: uniquePosts
        } AS hot_tag
        ORDER BY hot_tag.post_count DESC
        RETURN COLLECT(hot_tag) AS hot_tags
    ",
    )
    .param("ids", users_id)
}
