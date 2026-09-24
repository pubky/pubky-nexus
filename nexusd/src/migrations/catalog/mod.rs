//! Concrete migrations the daemon can register. The manager does not depend on these types.

use super::manager::{Migration, MigrationManager};

use collected_edges_backfill_1789344000::CollectedEdgesBackfill1789344000;
use post_content_index_author_setup_1780531200::PostContentIndexAuthorSetup1780531200;
use post_content_index_setup_1780444800::PostContentIndexSetup1780444800;
use remove_muted_1771718400::RemoveMuted1771718400;
use resource_node_setup_1774000000::ResourceNodeSetup1774000000;
use user_deleted_flag_1780617600::UserDeletedFlag1780617600;
use users_by_pk_reindex_1751635096::UsersByPkReindex1751635096;
use users_by_tags_index_backfill_1786924800::UsersByTagsIndexBackfill1786924800;

/// Registers this daemon's migrations with the manager.
///
/// After `db migration new`, add `Box::new(YourMigration)` here.
pub fn import_migrations(migration_manager: &mut MigrationManager) {
    let migrations: Vec<Box<dyn Migration>> = vec![
        // Note: Add your migrations here to be picked up by the manager
        Box::new(UsersByPkReindex1751635096),
        Box::new(RemoveMuted1771718400),
        Box::new(ResourceNodeSetup1774000000),
        Box::new(PostContentIndexSetup1780444800),
        Box::new(PostContentIndexAuthorSetup1780531200),
        // UserDeletedFlag must precede the users-by-tags backfill: the backfill
        // filters tombstones by `deleted`, which only this migration sets.
        Box::new(UserDeletedFlag1780617600),
        Box::new(UsersByTagsIndexBackfill1786924800),
        Box::new(CollectedEdgesBackfill1789344000),
    ];
    for migration in migrations {
        migration_manager.register(migration);
    }
}

// `db migration new` appends `pub mod …;` here.
// pub mod tag_counts_reset_1739459180;
pub mod collected_edges_backfill_1789344000;
pub mod post_content_index_author_setup_1780531200;
pub mod post_content_index_setup_1780444800;
pub mod remove_muted_1771718400;
pub mod resource_node_setup_1774000000;
pub mod user_deleted_flag_1780617600;
pub mod users_by_pk_reindex_1751635096;
pub mod users_by_tags_index_backfill_1786924800;
