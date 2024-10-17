use crate::db::connectors::neo4j::get_neo4j_graph;
use neo4rs::Query;

// Exec a graph query without a return
pub async fn exec_single_row(query: Query) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;
    let mut result = graph.execute(query).await?;
    result.next().await?;
    Ok(())
}

// Exec a graph query that has a single "boolean" return
pub async fn exec_boolean_row(
    query: Query,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let mut result;
    {
        let graph = get_neo4j_graph()?;
        let graph = graph.lock().await;
        result = graph.execute(query).await?;
    }
    let mut boolean = false;
    while let Some(row) = result.next().await? {
        boolean = row.get("boolean")?;
    }
    Ok(boolean)
}
