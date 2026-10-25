use crate::migrations::manager::Migration;
use async_trait::async_trait;
use nexus_common::models::event::resource_changes::ResourceChange;
use nexus_common::types::DynError;

/// Run after upgrading all writers to append resource changes. The seed retains
/// their concurrent appends and publishes a new epoch atomically. Older writers
/// do not append, so upgrade them before running this migration.
pub struct ResourceChangesSeed1792071211;

#[async_trait]
impl Migration for ResourceChangesSeed1792071211 {
    fn id(&self) -> &'static str {
        "ResourceChangesSeed1792071211"
    }
    fn is_multi_staged(&self) -> bool {
        false
    }
    async fn dual_write(_: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }
    async fn backfill(&self) -> Result<(), DynError> {
        let lines = ResourceChange::reseed().await?;
        tracing::info!(lines, "Seeded resource changes feed");
        Ok(())
    }
    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }
    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
