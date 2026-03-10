use crate::{db::DatabaseConfig, file::reader::DEFAULT_CONFIG_TOML};
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt::Debug, path::PathBuf, sync::OnceLock};

use super::{file::validate_and_expand_path, Level};

static DEFAULT_STACK_CONFIG: OnceLock<StackConfig> = OnceLock::new();

/// Intermediary struct used in deserializing [StackConfig] from `default.config.toml`
///
/// This is done because we don't know the full structure of the config.toml , but we
/// do know it contains a `[stack]` section with [StackConfig].
#[derive(Deserialize)]
struct DefaultConfig {
    stack: StackConfig,
}

fn deserialize_and_expand<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let path: PathBuf = Deserialize::deserialize(deserializer)?;
    validate_and_expand_path(path).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct StackConfig {
    pub log_level: Level,
    #[serde(deserialize_with = "deserialize_and_expand")]
    pub files_path: PathBuf,
    pub otlp_endpoint: Option<String>,
    pub db: DatabaseConfig,
}

/// Returns the default stack config parsed from `default.config.toml`
pub(crate) fn get_default_stack_config() -> &'static StackConfig {
    DEFAULT_STACK_CONFIG.get_or_init(|| {
        let config: DefaultConfig = toml::from_str(DEFAULT_CONFIG_TOML)
            .expect("embedded default.config.toml should be valid TOML");
        config.stack
    })
}

impl Default for StackConfig {
    /// Extracted from `default.config.toml`
    fn default() -> Self {
        get_default_stack_config().clone()
    }
}
