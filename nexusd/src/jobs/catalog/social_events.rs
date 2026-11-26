use crate::jobs::Job;
use async_trait::async_trait;
use nexus_common::{models::social_event, types::DynError};

pub struct EventOccurrencesJob;
#[async_trait]
impl Job for EventOccurrencesJob {
    fn name(&self) -> &'static str {
        "event-occurrences"
    }
    fn default_cron(&self) -> Option<&'static str> {
        Some("0 20 4 * * *")
    }
    async fn run(&self) -> Result<(), DynError> {
        Ok(social_event::maintain_occurrences(chrono::Utc::now()).await?)
    }
}
pub struct OrphanRsvpsJob;
#[async_trait]
impl Job for OrphanRsvpsJob {
    fn name(&self) -> &'static str {
        "orphan-rsvps"
    }
    fn default_cron(&self) -> Option<&'static str> {
        Some("0 40 4 * * SUN")
    }
    async fn run(&self) -> Result<(), DynError> {
        Ok(social_event::prune_orphan_rsvps().await?)
    }
}
