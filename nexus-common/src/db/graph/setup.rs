use crate::db::{get_neo4j_graph, graph::error::GraphResult, GraphError};
use neo4rs::query;
use tracing::info;

/// Ensure the Neo4j graph has the required constraints and indexes
pub async fn setup_graph() -> GraphResult<()> {
    // Define unique constraints
    let constraints = [
        "CREATE CONSTRAINT uniqueUserId IF NOT EXISTS FOR (u:User) REQUIRE u.id IS UNIQUE",
        "CREATE CONSTRAINT uniquePostId IF NOT EXISTS FOR (p:Post) REQUIRE p.id IS UNIQUE",
        "CREATE CONSTRAINT uniqueFileId IF NOT EXISTS FOR (f:File) REQUIRE (f.owner_id, f.id) IS UNIQUE",
        "CREATE CONSTRAINT uniqueHomeserverId IF NOT EXISTS FOR (hs:Homeserver) REQUIRE hs.id IS UNIQUE",
    ];

    // Create indexes
    let indexes = [
        "CREATE INDEX userIdIndex IF NOT EXISTS FOR (u:User) ON (u.id)",
        "CREATE INDEX postIdIndex IF NOT EXISTS FOR (p:Post) ON (p.id)",
        "CREATE INDEX postTimestampIndex IF NOT EXISTS FOR (p:Post) ON (p.indexed_at)",
        "CREATE INDEX postKindIndex IF NOT EXISTS FOR (p:Post) ON (p.kind)",
        "CREATE INDEX taggedLabelIndex IF NOT EXISTS FOR ()-[r:TAGGED]-() ON (r.label)",
        "CREATE INDEX taggedTimestampIndex IF NOT EXISTS FOR ()-[r:TAGGED]-() ON (r.indexed_at)",
        "CREATE INDEX fileIdIndex IF NOT EXISTS FOR (f:File) ON (f.owner_id, f.id)",
        "CREATE INDEX homeserverIdIndex IF NOT EXISTS FOR (hs:Homeserver) ON (hs.id)",
    ];

    let queries = constraints.iter().chain(indexes.iter());

    let graph = get_neo4j_graph()?;

    // Start an explicit transaction
    let txn = graph
        .start_txn()
        .await
        .map_err(|e| GraphError::Generic(format!("Failed to start transaction: {e}")))?;

    for &ddl in queries {
        graph.run(query(ddl)).await.map_err(|e| {
            GraphError::Generic(format!(
                "Failed to apply graph constraint/index '{ddl}': {e}"
            ))
        })?;
    }
    // Commit everything in one go
    txn.commit()
        .await
        .map_err(|e| GraphError::Generic(format!("Failed to commit transaction: {e}")))?;

    info!("Neo4j graph constraints and indexes have been applied successfully");

    Ok(())
}
