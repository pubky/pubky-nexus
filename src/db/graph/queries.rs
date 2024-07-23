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

// Retrive user node by id (pk)
pub fn get_user_by_id(user_id: &str) -> Query {
    query("MATCH (u:User {id: $id}) RETURN u").param("id", user_id)
}

pub fn profile_tags(user_id: &str) -> neo4rs::Query {
    query(
        "MATCH (u:User {id: $id})<-[t:TAGGED_AS]-(author:User)
           RETURN t.label AS tag .....",
    )
    .param("id", user_id)
}

pub fn profile_counts(user_id: &str) -> neo4rs::Query {
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

pub fn viewer_relationship(user_id: &str, viewer_id: &str) -> neo4rs::Query {
    query(
        "MATCH (u:User {id: $user_id})
         OPTIONAL MATCH (viewer:User {id: $viewer_id})
         RETURN EXISTS((viewer)-[:FOLLOWS]->(u)) AS following,
                EXISTS((u)-[:FOLLOWS]->(viewer)) AS followed_by,
                COUNT(u) > 0 AS user_exists,
                COUNT(viewer) > 0 AS viewer_exists",
    )
    .param("user_id", user_id)
    .param("viewer_id", viewer_id)
}
