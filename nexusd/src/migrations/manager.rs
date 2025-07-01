use async_trait::async_trait;
use chrono::Utc;
use neo4rs::{Graph, Query};
use nexus_common::{db::get_neo4j_graph, types::DynError};
use serde::{Deserialize, Serialize};
use std::{any::Any, sync::Arc};
use tokio::sync::Mutex;
use tracing::info;

use crate::migrations::utils::{self, generate_template};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPhase {
    DualWrite,
    Backfill,
    Cutover,
    Cleanup,
    Done,
}

impl MigrationPhase {
    fn next(&self) -> Option<MigrationPhase> {
        match self {
            MigrationPhase::DualWrite => Some(MigrationPhase::Backfill),
            MigrationPhase::Backfill => Some(MigrationPhase::Cutover),
            MigrationPhase::Cutover => Some(MigrationPhase::Cleanup),
            MigrationPhase::Cleanup => Some(MigrationPhase::Done),
            MigrationPhase::Done => None,
        }
    }

    fn to_string(&self) -> &str {
        match self {
            MigrationPhase::DualWrite => "dual_write",
            MigrationPhase::Backfill => "backfill",
            MigrationPhase::Cutover => "cutover",
            MigrationPhase::Cleanup => "cleanup",
            MigrationPhase::Done => "done",
        }
    }
}

#[async_trait]
pub trait Migration {
    fn id(&self) -> &'static str;
    /*
     * Should be marked as true if the migration is multi-staged.
     * Alternatively you can return false if the migration is single staged, this will cause the migration
     * to only run the backfill phase.
     */
    fn is_multi_staged(&self) -> bool;
    /*
     * This method should be implemented to write data to the new source.
     * For redis, if your struct is doing impl RedisOps for ExampleSturct,
     * you can add a impl RedisOps for MigrationExampleStruct in the migration file,
     * and use that to write to the new redis source.
     */
    async fn dual_write(data: Box<dyn Any + Send + 'static>) -> Result<(), DynError>
    where
        Self: Sized;
    /* Backfill is where the data is copied from the old source to the new source.
     * This is the most important phase of the migration.
     * You should make sure after this phase, the data in the new source is consistent with the old source.
     */
    async fn backfill(&self) -> Result<(), DynError>;
    /* This phase is where the cutover is done. This is where the application starts reading from the new source.
     * For graph, this might mean changing the application layer code, and removing the dual_write calls.
     * For redis, most of the time a simple rename command from the new key to the old key is enough.
     */
    async fn cutover(&self) -> Result<(), DynError>;
    /* This phase is where the old source is cleaned up.
     * For graph, this might mean deleting the old nodes and relationships.
     * For redis, this might mean deleting the old keys, if any is left.
     */
    async fn cleanup(&self) -> Result<(), DynError>;
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct MigrationNode {
    id: String,
    phase: MigrationPhase,
    created_at: i64,
    updated_at: i64,
}

const MIGRATION_PATH: &str = "nexusd/src/migrations/migrations_list/";

pub struct MigrationManager {
    graph: Arc<Mutex<Graph>>,
    migrations: Vec<Box<dyn Migration>>,
}

impl Default for MigrationManager {
    fn default() -> Self {
        let graph_connection = match get_neo4j_graph() {
            Ok(connection) => connection,
            Err(e) => panic!("Could not initialise migration manager: {e:?}"),
        };
        Self {
            graph: graph_connection,
            migrations: Vec::new(),
        }
    }
}

impl MigrationManager {
    pub async fn dual_write<T: Migration>(data: Box<dyn Any + Send>) -> Result<(), DynError> {
        T::dual_write(data).await
    }

    pub async fn new_migration(name: String) -> Result<(), DynError> {
        let now = Utc::now().timestamp();
        let snake_case_name = utils::to_snake_case(&name);
        let migration_file_name = format!("{snake_case_name}_{now}");
        let migration_template = generate_template(&migration_file_name);
        let file_path = format!("{}{}.rs", MIGRATION_PATH, &migration_file_name);
        tokio::fs::write(file_path.clone(), migration_template)
            .await
            .map_err(|err| {
                format!(
                    "Failed to create migration file at {}: error: {}",
                    file_path.as_str(),
                    err
                )
            })?;

        // append to migrations_list/mod.rs
        let mod_file_path = format!("{MIGRATION_PATH}mod.rs");
        let mod_file_content = format!("pub mod {migration_file_name};\n");
        let mut mod_file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(mod_file_path)
            .await?;
        tokio::io::AsyncWriteExt::write_all(&mut mod_file, mod_file_content.as_bytes()).await?;
        println!("Migration file created at {file_path}");
        Ok(())
    }

    pub fn register(&mut self, migration: Box<dyn Migration>) {
        self.migrations.push(migration);
    }

    pub async fn run(&mut self, migrations_backfill_ready: &[String]) -> Result<(), DynError> {
        // get all migrations from the database
        let stored_migrations = self.get_migrations().await?;
        // update any migration marked as ready for backfill
        for stored_migration in &stored_migrations {
            if migrations_backfill_ready.contains(&stored_migration.id) {
                self.update_migration_phase(&stored_migration.id, &MigrationPhase::Backfill)
                    .await?;
            }
        }

        // get all migrations from the database
        let stored_migrations = self.get_migrations().await?;
        // perform pending migrations
        for migration in &self.migrations {
            let migration_id = migration.id();
            let is_migration_multi_staged = migration.is_multi_staged();
            let mut stored_migration: Option<MigrationNode> = stored_migrations
                .iter()
                .find(|m| m.id == migration_id)
                .cloned();
            if stored_migration.is_none() {
                info!("Storing new migration {}...", migration_id);
                self.store_new_migration(migration_id, is_migration_multi_staged)
                    .await?;
                if is_migration_multi_staged {
                    continue;
                } else {
                    let now = Utc::now().timestamp_millis();
                    let new_migration_node = MigrationNode {
                        id: migration_id.to_string(),
                        phase: MigrationPhase::Backfill,
                        created_at: now,
                        updated_at: now,
                    };
                    stored_migration = Some(new_migration_node.clone());
                }
            }
            let stored_migration = stored_migration.unwrap();
            if stored_migration.phase == MigrationPhase::Done {
                info!("Migration {} is already done", migration_id);
                continue;
            }
            info!(
                "Migration {} is at phase {}",
                migration_id,
                stored_migration.phase.to_string()
            );

            match stored_migration.phase {
                MigrationPhase::Backfill => migration.backfill().await?,
                MigrationPhase::Cutover => {
                    migration.cutover().await?;
                }
                MigrationPhase::Cleanup => migration.cleanup().await?,
                _ => continue,
            }
            info!(
                "Migration {} completed phase {} successfully!",
                migration_id,
                stored_migration.phase.to_string()
            );
            let next_phase = match is_migration_multi_staged {
                true => stored_migration.phase.next(),
                false => Some(MigrationPhase::Done),
            };
            if let Some(next_phase) = next_phase {
                self.update_migration_phase(migration_id, &next_phase)
                    .await?;
            }
        }
        Ok(())
    }

    async fn get_migrations(&self) -> Result<Vec<MigrationNode>, DynError> {
        let query = Query::new("MATCH (m:Migration) RETURN COLLECT(m) as migrations".to_string());
        let mut result = self
            .graph
            .lock()
            .await
            .execute(query)
            .await
            .map_err(|e| e.to_string())?;

        match result.next().await {
            Ok(row) => match row {
                Some(row) => match row.get::<Vec<MigrationNode>>("migrations") {
                    Ok(migrations) => Ok(migrations),
                    Err(e) => Err(format!("GET ROW ERROR: {e:?}, {row:?}").into()),
                },
                None => Err("Migration Not found".into()),
            },
            Err(e) => Err(format!("GET MIGRATION ERROR: {e:?}").into()),
        }
    }

    async fn store_new_migration(&self, id: &str, is_multi_stage: bool) -> Result<(), DynError> {
        let initial_phase = match is_multi_stage {
            true => MigrationPhase::DualWrite,
            false => MigrationPhase::Backfill,
        };
        let query = Query::new(
            "MERGE (m:Migration {id: $id, phase: $phase, created_at: timestamp(), updated_at: 0})"
                .to_string(),
        )
        .param("id", id)
        .param("phase", initial_phase.to_string());

        self.graph.lock().await.run(query).await?;
        Ok(())
    }

    async fn update_migration_phase(
        &self,
        id: &str,
        phase: &MigrationPhase,
    ) -> Result<(), DynError> {
        let query = Query::new(
            "MERGE (m:Migration {id: $id}) SET m.phase = $phase, m.updated_at = timestamp()"
                .to_string(),
        )
        .param("id", id)
        .param("phase", phase.to_string());

        self.graph.lock().await.run(query).await?;
        Ok(())
    }
}
