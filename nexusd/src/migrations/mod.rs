#[allow(unused_imports)]
use manager::Migration;

pub mod builder;
pub mod manager;
mod migrations_list;
mod utils;

pub use builder::MigrationBuilder;
pub use manager::MigrationManager;

use crate::migrations::migrations_list::users_by_pk_reindex_1751635096::UsersByPkReindex1751635096;
use crate::migrations::migrations_list::remove_muted_1771718400::RemoveMuted1771718400;
/// Registers migrations with the `MigrationManager`
///
/// # Description
/// This function populates the migration manager with a list of migration tasks.
/// Each migration should be manually added to the `migrations` vector after it is
/// created in the `/db/migrations/migration_list` folder. The migration ID must be copied and
/// referenced in this function to ensure it is included in the execution process.
///
/// # Steps to Add a New Migration in pub fn import_migrations:
/// 1. Create a migration using the CLI: `cargo run -- db migration new DumpNotifications`
/// 2. Copy the migration struct name (e.g., `DumpNotifications1739459200`).
/// 3. Add it to the `migrations` vector as `Box::new(DumpNotifications1739459200)`.
/// 4. Ensure the migration is registered by calling `migration_manager.register(migration)`
///
/// # Example:
/// ```rust
/// let migrations: Vec<Box<dyn Migration>> = vec![
///     Box::new(DumpNotifications1739459200),
///     Box::new(AnotherMigration1739459201), // Add new migrations here
/// ];
/// ```
///
/// # Parameters
/// - `migration_manager`: A mutable reference to `MigrationManager` where migrations will be registered.
///
pub fn import_migrations(migration_manager: &mut MigrationManager) {
    let migrations: Vec<Box<dyn Migration>> = vec![
        // Note: Add your migrations here to be picked up by the manager
        Box::new(UsersByPkReindex1751635096),
        Box::new(RemoveMuted1771718400),
    ];
    for migration in migrations {
        migration_manager.register(migration);
    }
}
