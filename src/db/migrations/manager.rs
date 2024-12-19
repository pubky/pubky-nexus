use crate::{types::DynError, Config};
use axum::async_trait;
use chrono::Utc;
use neo4rs::{Graph, Query};
use serde::{Deserialize, Serialize};
use std::{any::Any, sync::Arc};
use tokio::sync::Mutex;

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

#[derive(Deserialize, Serialize, Clone)]
pub struct MigrationNode {
    id: String,
    phase: MigrationPhase,
    created_at: i64,
    updated_at: i64,
}

pub struct MigrationManager {
    graph: Arc<Mutex<Graph>>,
    migrations: Vec<Box<dyn Migration>>,
}

impl MigrationManager {
    pub fn new(graph: Arc<Mutex<Graph>>) -> Self {
        Self {
            graph,
            migrations: Vec::new(),
        }
    }

    pub async fn dual_write<T: Migration>(data: Box<dyn Any + Send>) -> Result<(), DynError> {
        T::dual_write(data).await
    }

    pub async fn new_migration(name: &str) -> Result<(), DynError> {
        let migration_name = format!("{}{}", name, Utc::now().timestamp());
        let migration_file_name = format!("{}_{}", to_snake_case(name), Utc::now().timestamp());
        let migration_template = get_migration_template(migration_name.as_str());
        let file_path = format!(
            "src/db/migrations/migrations_list/{}.rs",
            migration_file_name.as_str()
        );
        tokio::fs::write(file_path.clone(), migration_template).await?;

        // append to migrations_list/mod.rs
        let mod_file_path = "src/db/migrations/migrations_list/mod.rs";
        let mod_file_content = format!("pub mod {};\n", migration_file_name);
        let mut mod_file = tokio::fs::OpenOptions::new()
            .append(true)
            .open(mod_file_path)
            .await?;
        tokio::io::AsyncWriteExt::write_all(&mut mod_file, mod_file_content.as_bytes()).await?;
        println!("Migration file created at {}", file_path);
        Ok(())
    }

    pub fn register(&mut self, migration: Box<dyn Migration>) {
        self.migrations.push(migration);
    }

    pub async fn run(&self, config: &Config) -> Result<(), DynError> {
        // get all migrations from the database
        let stored_migrations = self.get_migrations().await?;
        // update any migration marked as ready for backfill
        for stored_migration in &stored_migrations {
            if config
                .migrations_backfill_ready
                .contains(&stored_migration.id)
            {
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
                println!("Storing new migration {}...", migration_id);
                self.store_new_migration(migration_id, is_migration_multi_staged)
                    .await?;
                if is_migration_multi_staged {
                    continue;
                } else {
                    let new_migration_node = MigrationNode {
                        id: migration_id.to_string(),
                        phase: MigrationPhase::Backfill,
                        created_at: Utc::now().timestamp_millis(),
                        updated_at: Utc::now().timestamp_millis(),
                    };
                    stored_migration = Some(new_migration_node.clone());
                }
            }
            let stored_migration = stored_migration.unwrap();
            if stored_migration.phase == MigrationPhase::Done {
                println!("Migration {} is already done", migration_id);
                continue;
            }
            println!(
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
            println!(
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
                    Err(e) => Err(e.into()),
                },
                None => Err("Migration Not found".into()),
            },
            Err(e) => Err(e.into()),
        }
    }

    async fn store_new_migration(&self, id: &str, is_multi_stage: bool) -> Result<(), DynError> {
        let initial_phase = match is_multi_stage {
            true => MigrationPhase::DualWrite,
            false => MigrationPhase::Backfill,
        };
        let query = Query::new(
            "MERGE (m:Migration {id: $id, phase: $phase, created_at: timestamp()})".to_string(),
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

fn get_migration_template(name: &str) -> String {
    format!(
        "use axum::async_trait;

use crate::db::migrations::manager::Migration;
use crate::types::DynError;

pub struct {name};

#[async_trait]
impl Migration for {name} {{
    fn id(&self) -> &'static str {{
        \"{name}\"
    }}  
        
    fn is_multi_staged(&self) -> bool {{
        true
    }}  
        
    async fn dual_write(data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {{
        // Implement your dual write logic here. Downcast data to your struct type.
        Ok(())
    }}  
    
    async fn backfill(&self) -> Result<(), DynError> {{
        // Your backfill logic here
        Ok(())
    }}  
                
    async fn cutover(&self) -> Result<(), DynError> {{
        // Your cutover logic here
        Ok(())  
    }}  
                    
    async fn cleanup(&self) -> Result<(), DynError> {{
        // Your cleanup logic here
        Ok(())
    }}
        
}}
",
        name = name
    )
}

fn to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let mut prev_was_upper = false;

    for (i, c) in input.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && !prev_was_upper {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
            prev_was_upper = true;
        } else {
            result.push(c);
            prev_was_upper = false;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    #[tokio_shared_rt::test(shared)]
    async fn test_to_snake_case() {
        assert_eq!(super::to_snake_case("CamelCase"), "camel_case");
        assert_eq!(super::to_snake_case("PascalCase"), "pascal_case");
        assert_eq!(super::to_snake_case("snake_case"), "snake_case");
        assert_eq!(super::to_snake_case("kebab-case"), "kebab-case");
        assert_eq!(super::to_snake_case("UPPERCASE"), "uppercase");
        assert_eq!(super::to_snake_case("lowercase"), "lowercase");
        assert_eq!(super::to_snake_case("12345"), "12345");
        assert_eq!(super::to_snake_case("snake_case_123"), "snake_case_123");
        assert_eq!(super::to_snake_case("UserNewField"), "user_new_field");
    }
}
