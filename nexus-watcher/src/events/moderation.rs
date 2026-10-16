use std::path::Path;
use std::sync::Arc;

use crate::errors::EventProcessorError;
use crate::events::handlers;
use crate::events::inputs::{Reference, ReferenceTarget};
use nexus_common::models::user::UserIngestor;
use nexus_common::WatcherConfig;
use pubky_social_specs::PubkyId;
use tracing::{debug, info};

pub struct Moderation {
    /// Moderator trusted user id
    pub id: PubkyId,
    /// Tags to be moderated (tagged content is deleted)
    pub tags: Vec<String>,
}

impl Moderation {
    pub fn from_config(config: &WatcherConfig) -> Arc<Self> {
        Arc::new(Self {
            id: config.moderation_id.clone(),
            tags: config.moderated_tags.clone(),
        })
    }

    /// Check if a tag should trigger deletion of the tagged content.
    ///
    /// Returns `true` if the tag was applied by the moderator and its `label`
    /// is a moderated tag label.
    pub fn should_delete(&self, label: &str, tagger_id: &PubkyId) -> bool {
        tagger_id == &self.id && self.tags.iter().any(|tag| tag == label)
    }

    /// Apply moderation to the resource a moderator tag points at.
    ///
    /// `label` is the moderator tag's label and `moderated` the reference it tags. Removes the
    /// resource its key names (post, tag, user or file). Posts are hard-deleted when edge-free
    /// or tombstoned when engagement edges remain. Any other target is a no-op.
    #[tracing::instrument(name = "moderation.apply", skip_all)]
    pub async fn apply_moderation(
        &self,
        label: &str,
        moderated: Reference,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        let Reference { uri, target } = moderated;
        match target {
            ReferenceTarget::Post(key) => {
                info!("Moderation tag '{label}' detected. Deleting post {uri}");
                // Same gate as user-issued DEL events: hard-delete when edge-free, tombstone
                // when any engagement edge remains (tags, bookmarks, replies, reposts, mentions).
                handlers::post::del(key, ingestor).await
            }
            ReferenceTarget::Tag { key, app } => {
                info!("Moderation tag '{label}' detected. Deleting tag {uri}");
                handlers::tag::del(key, app).await
            }
            ReferenceTarget::User(user_id) => {
                info!("Moderation tag '{label}' detected. Deleting user profile {user_id}");
                handlers::user::del(user_id).await
            }
            ReferenceTarget::File(key) => {
                info!("Moderation tag '{label}' detected. Deleting file {uri}");
                handlers::file::del(key, files_path).await
            }
            ReferenceTarget::Other => {
                debug!("Moderation tag '{label}' on {uri} names nothing Nexus deletes");
                Ok(())
            }
        }
    }
}
