use serde::{Deserialize, Serialize};

/// Scheduling config for a cron job, keyed under `[jobs.<name>]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct JobConfig {
    /// 7-field cron (`sec min hour dom month dow [year]`, seconds-first).
    /// `None` leaves the job unscheduled.
    pub cron: Option<String>,
}
