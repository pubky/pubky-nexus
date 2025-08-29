use anyhow::Result;
use neo4rs::{query, Query};
use nexus_common::db::fetch_key_from_graph;

pub async fn find_post_mentions(follower: &str, followee: &str) -> Result<Vec<String>> {
    let query = post_mention_query(follower, followee);
    let maybe_mentioned_list = fetch_key_from_graph(query, "mentioned_list").await.unwrap();

    if let Some(result) = maybe_mentioned_list {
        return Ok(result);
    }
    anyhow::bail!("Follow edge not found in Nexus graph");
}

fn post_mention_query(user_id: &str, post_id: &str) -> Query {
    query(
        "
        MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        OPTIONAL MATCH (p)-[:MENTIONED]->(mentioned_user:User)
        RETURN COLLECT(
            mentioned_user.id
        ) as mentioned_list
        ",
    )
    .param("author_id", user_id)
    .param("post_id", post_id)
}
