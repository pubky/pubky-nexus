use crate::jobs::Job;
use async_trait::async_trait;
use nexus_common::models::event::resource_changes::ResourceChange;
use nexus_common::types::DynError;

/// Regenerates the reference feed from current graph edges, retaining live appends.
/// `resource-changes-seed` refuses to replace a feed that holds lines with an empty snapshot,
/// which an empty or wrong graph would give; `resource-changes-seed-empty` publishes it, for a
/// graph whose references were all deleted. Neither may be scheduled ([`Job::schedulable`]):
/// every run resets every consumer, so an operator runs one on demand.
pub struct ResourceChangesSeedJob {
    pub allow_empty: bool,
}

#[async_trait]
impl Job for ResourceChangesSeedJob {
    fn name(&self) -> &'static str {
        if self.allow_empty {
            "resource-changes-seed-empty"
        } else {
            "resource-changes-seed"
        }
    }

    fn schedulable(&self) -> bool {
        false
    }

    async fn run(&self) -> Result<(), DynError> {
        let lines = ResourceChange::reseed(self.allow_empty).await?;
        tracing::info!(lines, "Resource changes feed seeded; epoch advanced");
        Ok(())
    }
}
