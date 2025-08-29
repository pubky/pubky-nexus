use anyhow::Result;
use neo4rs::{query, Query};
use nexus_common::db::fetch_key_from_graph;

pub async fn find_follow_relationship(follower: &str, followee: &str) -> Result<bool> {
    let query = user_following_query(follower, followee);

    let maybe_exists = fetch_key_from_graph(query, "exist").await.unwrap();

    if let Some(result) = maybe_exists {
        return Ok(result);
    }
    anyhow::bail!("Follow edge not found in Nexus graph");
}

fn user_following_query(follower: &str, followee: &str) -> Query {
    query(" RETURN EXISTS((:User {id: $follower})-[:FOLLOWS]->(:User {id: $followee})) AS exist")
        .param("followee", followee)
        .param("follower", follower)
}
