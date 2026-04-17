use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{db::get_neo4j_graph, db::graph::Query, types::DynError};
use tracing::info;

pub struct PostAuthorIdConstraint1776528000;

#[async_trait]
impl Migration for PostAuthorIdConstraint1776528000 {
    fn id(&self) -> &'static str {
        "PostAuthorIdConstraint1776528000"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        let graph = get_neo4j_graph()?;

        // Post identity is per-author `(author_id, post_id)`, not globally unique by id.
        // The old `uniquePostId` constraint enforced global uniqueness, which breaks as soon
        // as two users mint posts with the same client-generated id. This migration:
        //   1. Copies `author_id` onto each Post node from its AUTHORED edge, in batches.
        //   2. Drops the old single-column constraint.
        //   3. Installs the composite `(author_id, id)` constraint.
        //
        // The backfill uses `CALL { ... } IN TRANSACTIONS OF N ROWS` so each batch commits
        // independently — bounds memory/lock windows and makes progress resumable.
        let backfill_author_id = "\
             MATCH (u:User)-[:AUTHORED]->(p:Post) \
             WHERE p.author_id IS NULL \
             CALL { \
               WITH p, u \
               SET p.author_id = u.id \
             } IN TRANSACTIONS OF 10000 ROWS";

        let drop_old = "DROP CONSTRAINT uniquePostId IF EXISTS";

        let create_new = "CREATE CONSTRAINT uniquePostId IF NOT EXISTS \
             FOR (p:Post) REQUIRE (p.author_id, p.id) IS UNIQUE";

        for ddl in [backfill_author_id, drop_old, create_new] {
            info!("Applying: {ddl}");
            graph
                .run(Query::new("post_author_id_constraint", ddl))
                .await
                .map_err(|e| -> DynError {
                    Box::new(std::io::Error::other(format!(
                        "Failed to apply '{ddl}': {e}"
                    )))
                })?;
        }

        info!("Post composite identity constraint applied successfully");
        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
