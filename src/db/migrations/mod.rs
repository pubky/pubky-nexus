use std::sync::Arc;

use migrations_list::tag_counts_reset_1739459180::TagCountsReset1739459180;
use neo4rs::Graph;
use tokio::sync::Mutex;

use crate::MigrationManager;

pub mod manager;
mod migrations_list;
mod utils;

pub fn get_migration_manager(graph: Arc<Mutex<Graph>>) -> MigrationManager {
    let mut migration_manager = MigrationManager::new(graph);
    // Add your migrations here to be picked up by the manager. Example:
    migration_manager.register(Box::new(TagCountsReset1739459180));
    migration_manager
    //MigrationManager::new(graph)
}
