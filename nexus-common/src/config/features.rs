use serde::{Deserialize, Serialize};

use crate::db::kv::RedisResult;
use crate::models::post::trust_filter;

/// App feature switches (`[features]`), all off by default. The watcher reads
/// them at startup and stores them in Redis, where every service follows them:
/// the API and the jobs don't read `[features]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeaturesConfig {
    /// Hide posts by authors outside the trust ranking from the streams every
    /// viewer shares.
    pub hide_unranked_authors: bool,
}

impl FeaturesConfig {
    /// Stores the switches in Redis, where every service reads them.
    pub async fn store(&self) -> RedisResult<()> {
        trust_filter::switch(self.hide_unranked_authors).await
    }

    /// Stores the switches, then brings the stored data in line with them, so a
    /// feature toggled since the last start takes effect now. Needs the stack
    /// set up. Fails only if the switches can't be stored: a failed update is
    /// logged and left to the next trust recompute, which retries it.
    pub async fn toggle(&self) -> RedisResult<()> {
        self.store().await?;
        if let Err(error) = trust_filter::reconcile().await {
            tracing::error!(%error, "Failed to bring the shared post sets in line with the features");
        }
        Ok(())
    }
}
