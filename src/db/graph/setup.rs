use crate::db::connectors::neo4j::get_neo4j_graph;
use neo4rs::query;

// Set graph constraints if they do not already exist
pub async fn setup_graph() -> Result<(), Box<dyn std::error::Error>> {
    let constraints = [
        "CREATE CONSTRAINT uniqueUserId IF NOT EXISTS FOR (u:User) REQUIRE u.id IS UNIQUE",
        "CREATE CONSTRAINT uniquePostId IF NOT EXISTS FOR (p:Post) REQUIRE p.id IS UNIQUE",
    ];

    let indexes = [
        "CREATE INDEX userIdIndex IF NOT EXISTS FOR (u:User) ON (u.id)",
        "CREATE INDEX postIdIndex IF NOT EXISTS FOR (p:Post) ON (p.id)",
        "CREATE INDEX taggedLabelIndex IF NOT EXISTS FOR ()-[r:TAGGED]-() ON (r.label)",
    ];

    let queries = constraints.iter().chain(indexes.iter());

    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;
    for q in queries {
        graph.run(query(q)).await?;
    }

    Ok(())
}
