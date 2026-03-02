use anyhow::Result;
use nexus_common::db::fetch_key_from_graph;
use nexus_common::db::graph::query;

pub async fn find_mute_relationship(muter: &str, mutee: &str) -> Result<bool> {
    let query =
        query("RETURN EXISTS((:User {id: $muter})-[:MUTED]->(:User {id: $mutee})) AS exist")
            .param("muter", muter)
            .param("mutee", mutee);

    let maybe_exists = fetch_key_from_graph(query, "exist").await.unwrap();

    if let Some(result) = maybe_exists {
        return Ok(result);
    }
    anyhow::bail!("Mute edge not found in Nexus graph");
}
