use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{db::get_neo4j_graph, db::setup::apply_schema_ddl, types::DynError};
use tracing::info;

pub struct ResourceNodeSetup1774000000;

#[async_trait]
impl Migration for ResourceNodeSetup1774000000 {
    fn id(&self) -> &'static str {
        "ResourceNodeSetup1774000000"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        let graph = get_neo4j_graph()?;

        let ddl_statements = [
            "CREATE CONSTRAINT uniqueResourceId IF NOT EXISTS FOR (r:Resource) REQUIRE r.id IS UNIQUE",
            "CREATE INDEX resourceSchemeIndex IF NOT EXISTS FOR (r:Resource) ON (r.scheme)",
            "CREATE INDEX taggedAppIndex IF NOT EXISTS FOR ()-[r:TAGGED]-() ON (r.app)",
        ];

        for ddl in ddl_statements {
            info!("Applying DDL: {ddl}");
            apply_schema_ddl(graph.as_ref(), "resource_ddl", ddl)
                .await
                .map_err(|e| -> DynError { Box::new(std::io::Error::other(e.to_string())) })?;
        }

        info!("Resource node DDL applied successfully");
        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        // No cutover needed — DDL is applied in backfill
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        // No cleanup needed
        Ok(())
    }
}
