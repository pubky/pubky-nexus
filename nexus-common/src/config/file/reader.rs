use crate::types::DynError;
use std::ffi::OsStr;
use std::path::{Component, PathBuf};

/// Path to default nexusd config file. Defaults to ~/.pubky-nexus
pub const DEFAULT_HOME_DIR: &str = ".pubky-nexus";
pub(crate) const DEFAULT_CONFIG_TOML: &str = include_str!("../../../default.config.toml");
/// The sole configuration file name recognized by nexus
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// Expands the data directory to the home directory if it starts with "~"
/// Return the full path to the data directory
pub fn try_expand_home_dir(path: PathBuf) -> Result<PathBuf, DynError> {
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
