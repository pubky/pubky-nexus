use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::types::DynError;
use neo4rs::Query;
use serde::de::DeserializeOwned;

// Exec a graph query without a return
pub async fn exec_single_row(query: Query) -> Result<(), DynError> {
    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;
    let mut result = graph.execute(query).await?;
    result.next().await?;
    Ok(())
}

// Exec a graph query that has a single "boolean" return
pub async fn exec_boolean_row(query: Query) -> Result<bool, DynError> {
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

// Exec a graph query that has a single "boolean" return
pub async fn temp_exec_boolean_row(query: Query) -> Result<Option<bool>, DynError> {
    let mut result;
    {
        let graph = get_neo4j_graph()?;
        let graph = graph.lock().await;
        result = graph.execute(query).await?;
    }
    let mut exist = None;
    println!("QUERY: Check if the graph retur a ROW");
    while let Some(row) = result.next().await? {
        println!("Graph return a row");
        let result: bool = row.get("boolean")?;
        exist = Some(result);

    }
    Ok(exist)
}

// Generic function to retrieve data from Neo4J
pub async fn retrieve_from_graph<T>(query: Query, key: &str) -> Result<Option<T>, DynError>
where
    // Key point: DeserializeOwned ensures we can deserialize into any type that implements it
    T: DeserializeOwned + Send + Sync,
{
    let mut result;
    {
        let graph = get_neo4j_graph()?;
        let graph = graph.lock().await;
        result = graph.execute(query).await?;
    }

    if let Some(row) = result.next().await? {
        let data: T = row.get(key)?;
        return Ok(Some(data));
    }

    Ok(None)
}
