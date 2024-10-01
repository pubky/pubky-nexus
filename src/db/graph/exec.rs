use crate::db::connectors::neo4j::get_neo4j_graph;
use neo4rs::Query;
use serde::de::DeserializeOwned;

pub async fn exec_single_row(query: Query) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;
    let mut result = graph.execute(query).await?;
    result.next().await?;
    Ok(())
}

// Generic function to retrieve data from Neo4J
pub async fn retrieve_from_graph<T>(
    query: Query,
    key: &str,
) -> Result<Option<T>, Box<dyn std::error::Error + Send + Sync>>
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

// Generic function to retrieve data from Neo4J
pub async fn retrieve_many_from_graph<T>(
    query: Query,
    key: &str,
) -> Result<Vec<T>, Box<dyn std::error::Error + Send + Sync>>
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

    let mut rows = vec![];
    while let Some(row) = result.next().await? {
        log::error!("row: {:?}", row);
        rows.push(row.get(key)?);
    }

    Ok(rows)
}
