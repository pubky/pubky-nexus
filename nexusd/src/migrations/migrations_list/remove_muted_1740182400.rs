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
        // 1. Remove all :MUTED relationships from Neo4j in batches
        let graph = get_neo4j_graph()?;
        let mut total_deleted: i64 = 0;

        loop {
            let query = neo4rs::query(
                "MATCH ()-[r:MUTED]->() WITH r LIMIT 10000 DELETE r RETURN count(r) AS deleted",
            );
            let mut result = graph.execute(query).await?;

            let deleted: i64 = match result.next().await? {
                Some(row) => row.get("deleted").unwrap_or(0),
                None => 0,
            };

            total_deleted += deleted;

            if deleted == 0 {
                break;
            }

            info!(
                "RemoveMuted migration: deleted batch of {} MUTED relationships ({} total so far)",
                deleted, total_deleted
            );
        }

        info!(
            "RemoveMuted migration: deleted {} MUTED relationships from graph",
            total_deleted
        );

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
