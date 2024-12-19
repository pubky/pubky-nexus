use std::sync::Arc;

use neo4rs::Graph;
use tokio::sync::Mutex;

use crate::MigrationManager;

pub mod manager;
mod migrations_list;

pub fn get_migration_manager(graph: Arc<Mutex<Graph>>) -> MigrationManager {
    // let migration_manager = MigrationManager::new(graph);
    // migration_manager.register(Box::new(MigrationX));
    MigrationManager::new(graph)
}
