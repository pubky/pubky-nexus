use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::types::DynError;
use neo4rs::Query;
use serde::de::DeserializeOwned;

/// Represents the outcome of a mutation-like query in the graph database.
#[derive(Debug)]
pub enum OperationOutcome {
    /// The query found and updated an existing node/relationship.
    Updated,
    /// This variant represents a structural mutation where the node/relationship
    /// did not exist before the operation (creation) or no longer exists after the operation (deletion)
    CreatedOrDeleted,
    /// A required node/relationship was not found, indicating a missing dependency
    /// (often due to the node/relationship not yet being indexed or otherwise unavailable).
    Pending,
}

/// Executes a graph query expected to return exactly one row containing a boolean column named
/// "flag". Interprets the boolean as follows:
///
/// - `true` => Returns [`OperationOutcome::Updated`]
/// - `false` => Returns [`OperationOutcome::CreatedOrDeleted`]
///
/// If no rows are returned, this function returns [`OperationOutcome::Pending`], typically
/// indicating a missing dependency or an unmatched query condition.
pub async fn execute_graph_operation(query: Query) -> Result<OperationOutcome, DynError> {
    let mut result;
    {
        let graph = get_neo4j_graph()?;
        let graph = graph.lock().await;
        result = graph.execute(query).await?;
    }

    match result.next().await? {
        // The "flag" field indicates a specific condition in the query
        Some(row) => match row.get("flag")? {
            true => Ok(OperationOutcome::Updated),
            false => Ok(OperationOutcome::CreatedOrDeleted),
        },
        None => Ok(OperationOutcome::Pending),
    }
}

// Exec a graph query without a return
pub async fn exec_single_row(query: Query) -> Result<(), DynError> {
    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;
    let mut result = graph.execute(query).await?;
    result.next().await?;
    Ok(())
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
