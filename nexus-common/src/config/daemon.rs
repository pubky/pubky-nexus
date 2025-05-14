use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::{file::ConfigLoader, ApiConfig, StackConfig, WatcherConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub watcher: WatcherConfig,
    pub stack: StackConfig,
}

#[async_trait]
impl ConfigLoader<DaemonConfig> for DaemonConfig {}
