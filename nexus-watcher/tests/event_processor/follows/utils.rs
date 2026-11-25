use anyhow::Result;
use nexus_common::db::fetch_key_from_graph;
use nexus_common::db::graph::Query;

pub async fn find_follow_relationship(follower: &str, followee: &str) -> Result<bool> {
    let query = user_following_query(follower, followee);

    let maybe_exists = fetch_key_from_graph(query, "exist").await.unwrap();

    if let Some(result) = maybe_exists {
        return Ok(result);
    }
    anyhow::bail!("Follow edge not found in Nexus graph");
}

fn user_following_query(follower: &str, followee: &str) -> Query {
    Query::new(
        "user_following_query",
        "RETURN EXISTS((:User {id: $follower})-[:FOLLOWS]->(:User {id: $followee})) AS exist",
    )
    .param("followee", followee)
    .param("follower", follower)
}

/// The `indexed_at` of the `FOLLOWS` edge from `follower` to `followee`.
pub async fn find_follow_indexed_at(follower: &str, followee: &str) -> Result<i64> {
    let query = Query::new(
        "follow_indexed_at",
        "MATCH (:User {id: $follower})-[r:FOLLOWS]->(:User {id: $followee})
         RETURN r.indexed_at AS indexed_at",
    )
    .param("followee", followee)
    .param("follower", follower);

    match fetch_key_from_graph(query, "indexed_at").await? {
        Some(indexed_at) => Ok(indexed_at),
        None => anyhow::bail!("Follow edge not found in Nexus graph"),
    }
}
