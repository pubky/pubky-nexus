use crate::types::DynError;
use std::ffi::OsStr;
use std::path::{Component, PathBuf};

/// Path to default nexusd config file. Defaults to ~/.pubky-nexus
///
/// See [default_config_dir_path] to use this as [PathBuf]
pub const DEFAULT_HOME_DIR: &str = ".pubky-nexus";
/// The default config TOML content embedded in the binary
pub const DEFAULT_CONFIG_TOML: &str = include_str!("../../../default.config.toml");
/// The sole configuration file name recognized by nexus
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// Returns [DEFAULT_HOME_DIR] as [PathBuf], relative to the home directory
pub fn default_config_dir_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(DEFAULT_HOME_DIR)
}

/// If the path starts with a "~", this expands the "~" to the full home directory path.
///
/// If the path points to a file, this throws an error.
pub fn validate_and_expand_path(path: PathBuf) -> Result<PathBuf, DynError> {
    let mut expanded_path = path.clone();

    if let Some(first) = path.components().next() {
        if first == Component::Normal(OsStr::new("~")) {
            if let Some(home) = dirs::home_dir() {
                // drop the "~" prefix and re-join
                let without_tilde = path.iter().skip(1).collect::<PathBuf>();
                expanded_path = home.join(without_tilde);
            }
        }
    }

    if expanded_path.exists() && expanded_path.is_file() {
        return Err(format!(
            "Specified path points to a file. Create a new path with `mkdir -p folder_path` or point to a directory: {}",
            path.display()
        )
        .into());
    }

    Ok(expanded_path)
}
