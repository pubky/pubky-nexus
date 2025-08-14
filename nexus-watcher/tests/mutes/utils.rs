use anyhow::Result;
use neo4rs::query;
use nexus_common::db::retrieve_from_graph;

pub async fn find_mute_relationship(muter: &str, mutee: &str) -> Result<bool> {
    let query =
        query("RETURN EXISTS((:User {id: $muter})-[:MUTED]->(:User {id: $mutee})) AS exist")
            .param("muter", muter)
            .param("mutee", mutee);

    let maybe_exists = retrieve_from_graph(query, "exist").await.unwrap();

    if let Some(result) = maybe_exists {
        return Ok(result);
    }
    anyhow::bail!("Mute edge not found in Nexus graph");
}
