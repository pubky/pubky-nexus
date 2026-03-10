use crate::db::{get_neo4j_graph, graph::error::GraphResult};
use neo4rs::{Query, Row};
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
    MissingDependency,
}

/// Executes a graph query expected to return exactly one row containing a boolean column named
/// "flag". Interprets the boolean as follows:
///
/// - `true` => Returns [`OperationOutcome::Updated`]
/// - `false` => Returns [`OperationOutcome::CreatedOrDeleted`]
///
/// If no rows are returned, this function returns [`OperationOutcome::MissingDependency`], typically
/// indicating a missing dependency or an unmatched query condition.
pub async fn execute_graph_operation(query: Query) -> GraphResult<OperationOutcome> {
    // The "flag" field indicates a specific condition in the query
    let maybe_flag = fetch_key_from_graph(query, "flag").await?;
    match maybe_flag {
        Some(true) => Ok(OperationOutcome::Updated),
        Some(false) => Ok(OperationOutcome::CreatedOrDeleted),
        None => Ok(OperationOutcome::MissingDependency),
    }
}

/// Exec a graph query without a return
pub async fn exec_single_row(query: Query) -> GraphResult<()> {
    let graph = get_neo4j_graph()?;
    let mut result = graph.execute(query).await?;
    result.next().await?;
    Ok(())
}

pub async fn fetch_row_from_graph(query: Query) -> GraphResult<Option<Row>> {
    let graph = get_neo4j_graph()?;

    let mut result = graph.execute(query).await?;

    result.next().await.map_err(Into::into)
}

pub async fn fetch_all_rows_from_graph(query: Query) -> GraphResult<Vec<Row>> {
    let graph = get_neo4j_graph()?;

    let mut result = graph.execute(query).await?;
    let mut rows = Vec::new();

    while let Some(row) = result.next().await? {
        rows.push(row);
    }

    Ok(rows)
}

/// Fetch the value of type T mapped to a specific key from the first row of a graph query's result
pub async fn fetch_key_from_graph<T>(query: Query, key: &str) -> GraphResult<Option<T>>
where
    // Key point: DeserializeOwned ensures we can deserialize into any type that implements it
    T: DeserializeOwned + Send + Sync,
{
    let maybe_row = fetch_row_from_graph(query).await?;

    let Some(row) = maybe_row else {
        return Ok(None);
    };

    row.get(key)
        .map(Some)
        .map_err(Into::into)
        .inspect_err(|e| tracing::error!("Failed to get {key} from query result: {e}"))
}
