use crate::db::connectors::neo4j::get_neo4j_graph;
use neo4rs::Query;

pub async fn exec_single_row(query: Query) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;
    let mut result = graph.execute(query).await?;
    result.next().await?;
    Ok(())
}
