use std::path::PathBuf;

use nexus_common::WatcherConfig;

use super::Moderation;

pub struct EventContext {
    pub moderation: Moderation,

    /// Nexus-local directory path where certain events may store data on Nexus.
    ///
    /// Moderation may trigger the deletion of such data for specific events.
    pub files_path: PathBuf,
}

impl EventContext {
    pub(crate) fn from_config(config: &WatcherConfig) -> Self {
        EventContext {
            moderation: Moderation {
                id: config.moderation_id.clone(),
                tags: config.moderated_tags.clone(),
            },
            files_path: config.stack.files_path.clone(),
        }
    }
}
