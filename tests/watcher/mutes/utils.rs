use anyhow::Result;
use neo4rs::query;
use pubky_nexus::get_neo4j_graph;

pub async fn find_mute_relationship(muter: &str, mutee: &str) -> Result<bool> {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query =
            query("RETURN EXISTS((:User {id: $muter})-[:MUTED]->(:User {id: $mutee})) AS exist")
                .param("muter", muter)
                .param("mutee", mutee);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(result) = row.unwrap().get::<bool>("exist") {
        return Ok(result);
    }
    anyhow::bail!("Mute edge not found in Nexus graph");
}
