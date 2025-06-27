use crate::{db::get_neo4j_graph, types::DynError};
use neo4rs::query;
use tracing::info;

/// Ensure the Neo4j graph has the required constraints and indexes
pub async fn setup_graph() -> Result<(), DynError> {
    // Define unique constraints
    let constraints = [
        "CREATE CONSTRAINT uniqueUserId IF NOT EXISTS FOR (u:User) REQUIRE u.id IS UNIQUE",
        "CREATE CONSTRAINT uniquePostId IF NOT EXISTS FOR (p:Post) REQUIRE p.id IS UNIQUE",
        "CREATE CONSTRAINT uniqueFileId IF NOT EXISTS FOR (f:File) REQUIRE (f.owner_id, f.id) IS UNIQUE",
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
    ];

    let queries = constraints.iter().chain(indexes.iter());

    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;

    // Start an explicit transaction
    let txn = graph
        .start_txn()
        .await
        .map_err(|e| format!("Failed to start transaction: {e}"))?;

    for &ddl in queries {
        if let Err(err) = graph.run(query(ddl)).await {
            return Err(format!("Failed to apply graph constraints/indexes: {err}").into());
        }
    }
    // Commit everything in one go
    txn.commit()
        .await
        .map_err(|e| format!("Failed to commit the transaction: {e}"))?;

    info!("Neo4j graph constraints and indexes have been applied successfully");

    Ok(())
}
