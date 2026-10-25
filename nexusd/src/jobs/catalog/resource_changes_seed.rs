use crate::jobs::Job;
use async_trait::async_trait;
use nexus_common::models::event::resource_changes::ResourceChange;
use nexus_common::types::DynError;

/// Regenerates the reference feed from current graph edges, retaining live appends.
pub struct ResourceChangesSeedJob;

#[async_trait]
impl Job for ResourceChangesSeedJob {
    fn name(&self) -> &'static str {
        "resource-changes-seed"
    }

    async fn run(&self) -> Result<(), DynError> {
        let lines = ResourceChange::reseed().await?;
        tracing::info!(lines, "Resource changes feed seeded; epoch advanced");
        Ok(())
    }
}
