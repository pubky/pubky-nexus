use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// What a Nexus stores of the objects it indexes.
///
/// - `full` keeps everything: post content, profiles, files and their variants.
/// - `light` keeps the social graph and every link, but none of the content people
///   wrote or uploaded. Clients fetch that content from the owner's homeserver.
///
/// A database is indexed in one mode for its whole life: see
/// [`crate::StackManager::ensure_mode_lock`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum NexusMode {
    #[default]
    Full,
    Light,
}

impl NexusMode {
    pub fn is_light(self) -> bool {
        self == NexusMode::Light
    }

    /// Parses the value stored in the mode lock.
    pub fn from_stored(value: &str) -> Option<Self> {
        match value {
            "full" => Some(NexusMode::Full),
            "light" => Some(NexusMode::Light),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            NexusMode::Full => "full",
            NexusMode::Light => "light",
        }
    }

    /// Decides the mode a database is locked to, given what the lock holds (`stored`),
    /// whether the graph already holds users (`has_data`) and what this process is
    /// configured with (`configured`).
    ///
    /// A database with no lock takes the configured mode if it is empty. One that already
    /// holds data predates the lock, so it was indexed in full mode. Returns the mode to
    /// store, or an error when it differs from the configured one.
    pub fn resolve_lock(
        stored: Option<NexusMode>,
        has_data: bool,
        configured: NexusMode,
    ) -> Result<NexusMode, ModeMismatch> {
        let locked = match stored {
            Some(mode) => mode,
            None if has_data => NexusMode::Full,
            None => configured,
        };
        if locked == configured {
            Ok(locked)
        } else {
            Err(ModeMismatch { locked, configured })
        }
    }
}

impl fmt::Display for NexusMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The database was indexed in a different mode than the one configured.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ModeMismatch {
    pub locked: NexusMode,
    pub configured: NexusMode,
}

impl fmt::Display for ModeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "this database was indexed in {locked} mode but the config sets mode = \"{configured}\". \
             A mode cannot change on an existing database: either set mode = \"{locked}\", or run \
             `nexusd db clear --yes` and let the watcher re-index from the homeservers in {configured} mode",
            locked = self.locked,
            configured = self.configured,
        )
    }
}

/// The same text as `Display`: `nexusd` returns this error from `main`, which prints it
/// with `Debug`, and the operator needs the instructions, not the field values.
impl fmt::Debug for ModeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for ModeMismatch {}

#[cfg(test)]
mod tests {
    use super::NexusMode::{Full, Light};
    use super::*;

    #[test]
    fn parses_lowercase_and_defaults_to_full() {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(default)]
            mode: NexusMode,
        }
        let light: Wrapper = toml::from_str("mode = \"light\"").unwrap();
        assert_eq!(light.mode, Light);
        let absent: Wrapper = toml::from_str("").unwrap();
        assert_eq!(absent.mode, Full);
        assert!(toml::from_str::<Wrapper>("mode = \"Light\"").is_err());
    }

    #[test]
    fn stored_values_round_trip() {
        for mode in [Full, Light] {
            assert_eq!(NexusMode::from_stored(mode.as_str()), Some(mode));
        }
        assert_eq!(NexusMode::from_stored("pruned"), None);
    }

    #[test]
    fn empty_database_takes_the_configured_mode() {
        assert_eq!(NexusMode::resolve_lock(None, false, Light), Ok(Light));
        assert_eq!(NexusMode::resolve_lock(None, false, Full), Ok(Full));
    }

    #[test]
    fn unlocked_database_with_data_is_full() {
        assert_eq!(NexusMode::resolve_lock(None, true, Full), Ok(Full));
        assert_eq!(
            NexusMode::resolve_lock(None, true, Light),
            Err(ModeMismatch {
                locked: Full,
                configured: Light
            })
        );
    }

    /// `nexusd` returns errors from `main`, which prints them with `Debug`, so `Debug` must
    /// carry the same instructions an operator gets from `Display`.
    #[test]
    fn mode_mismatch_debug_is_its_display() {
        let err = ModeMismatch {
            locked: Light,
            configured: Full,
        };
        assert_eq!(format!("{err:?}"), format!("{err}"));
        assert!(format!("{err:?}").contains("nexusd db clear --yes"));
    }

    #[test]
    fn stored_lock_must_match_the_config() {
        assert_eq!(NexusMode::resolve_lock(Some(Light), true, Light), Ok(Light));
        assert!(NexusMode::resolve_lock(Some(Light), true, Full).is_err());
        assert!(NexusMode::resolve_lock(Some(Full), false, Light).is_err());
    }
}
