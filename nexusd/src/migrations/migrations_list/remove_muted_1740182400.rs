use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{
    db::{get_neo4j_graph, kv::delete_keys_by_pattern},
    types::DynError,
};
use tracing::info;

pub struct RemoveMuted1740182400;

#[async_trait]
impl Migration for RemoveMuted1740182400 {
    fn id(&self) -> &'static str {
        "RemoveMuted1740182400"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // 1. Remove all :MUTED relationships from Neo4j
        let graph = get_neo4j_graph()?;
        let query = neo4rs::query("MATCH ()-[r:MUTED]->() DELETE r RETURN count(r) AS deleted");
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let deleted: i64 = row.get("deleted").unwrap_or(0);
            info!(
                "RemoveMuted migration: deleted {} MUTED relationships from graph",
                deleted
            );
        }

        // 2. Remove all Muted:* keys from Redis
        let deleted = delete_keys_by_pattern("Muted:*").await?;
        info!(
            "RemoveMuted migration: deleted {} Muted keys from Redis",
            deleted
        );

        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
