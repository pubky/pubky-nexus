use serde::{Deserialize, Serialize};

/// App features that can be switched off (`[features]`). Every service reads
/// it, so keep it the same in each one's config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeaturesConfig {
    /// Hide posts by authors outside the trust ranking from the streams every
    /// viewer shares.
    pub hide_unranked_authors: bool,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            hide_unranked_authors: true,
        }
    }
}

impl FeaturesConfig {
    /// Switches the features on or off for this process.
    pub fn apply(&self) {
        crate::models::post::trust_filter::set_enabled(self.hide_unranked_authors);
    }

    /// Switches the features on or off, then brings the stored data in line
    /// with them, so a feature toggled since the last start takes effect now.
    /// Needs the stack set up. A failure is logged and left to the next trust
    /// recompute, which retries it.
    pub async fn toggle(&self) {
        self.apply();
        if let Err(error) = crate::models::post::trust_filter::reconcile().await {
            tracing::error!(%error, "Failed to bring the shared post sets in line with the features");
        }
    }
}
