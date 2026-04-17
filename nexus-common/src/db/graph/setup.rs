use crate::db::get_neo4j_graph;
use crate::db::graph::error::{GraphError, GraphResult};
use crate::db::graph::Query;
use tokio::sync::OnceCell;
use tracing::info;

static GRAPH_SETUP: OnceCell<()> = OnceCell::const_new();

/// Ensure the Neo4j graph has the required constraints and indexes.
///
/// Uses a `OnceCell` so that concurrent callers (e.g. API + watcher starting
/// in parallel) only execute DDL once; the second caller awaits the first.
pub async fn setup_graph() -> GraphResult<()> {
    GRAPH_SETUP
        .get_or_try_init(setup_graph_inner)
        .await
        .copied()
}

async fn setup_graph_inner() -> GraphResult<()> {
    // Define unique constraints
    let constraints = [
        "CREATE CONSTRAINT uniqueUserId IF NOT EXISTS FOR (u:User) REQUIRE u.id IS UNIQUE",
        "CREATE CONSTRAINT uniquePostId IF NOT EXISTS FOR (p:Post) REQUIRE (p.author_id, p.id) IS UNIQUE",
        "CREATE CONSTRAINT uniqueFileId IF NOT EXISTS FOR (f:File) REQUIRE (f.owner_id, f.id) IS UNIQUE",
        "CREATE CONSTRAINT uniqueHomeserverId IF NOT EXISTS FOR (hs:Homeserver) REQUIRE hs.id IS UNIQUE",
        "CREATE CONSTRAINT uniqueResourceId IF NOT EXISTS FOR (r:Resource) REQUIRE r.id IS UNIQUE",
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
        "CREATE INDEX resourceSchemeIndex IF NOT EXISTS FOR (r:Resource) ON (r.scheme)",
        "CREATE INDEX taggedAppIndex IF NOT EXISTS FOR ()-[r:TAGGED]-() ON (r.app)",
    ];

    let queries = constraints.iter().chain(indexes.iter());

    let graph = get_neo4j_graph()?;

    for &ddl in queries {
        graph.run(Query::new("setup_ddl", ddl)).await.map_err(|e| {
            GraphError::Generic(format!(
                "Failed to apply graph constraint/index '{ddl}': {e}"
            ))
        })?;
    }

    info!("Neo4j graph constraints and indexes have been applied successfully");

    Ok(())
}
