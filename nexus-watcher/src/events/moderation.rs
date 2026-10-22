use std::path::Path;
use std::sync::Arc;

use crate::errors::EventProcessorError;
use crate::events::handlers;
use crate::events::inputs::{Reference, ReferenceTarget};
use crate::events::route::takedown_key;
use nexus_common::models::takedown::record_takedown;
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
    /// or tombstoned when engagement edges remain. Any other target is a no-op. `tag_uri` is
    /// the moderator tag's own address, kept with the takedown.
    #[tracing::instrument(name = "moderation.apply", skip_all)]
    pub async fn apply_moderation(
        &self,
        label: &str,
        moderated: Reference,
        tag_uri: &str,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        let Reference { uri, target } = moderated;
        // TODO(specs-migration): a `Resource` tag is never taken down. A moderator tag on a
        // foreign resource removes nothing, and a file takedown deletes the file but leaves the
        // `Resource` tags on its address.
        match target {
            // A profile is not remembered: its address is the user's, and a moderated user may
            // come back by writing a new profile.
            ReferenceTarget::User(user_id) => {
                info!("Moderation tag '{label}' detected. Deleting user profile {user_id}");
                handlers::user::del(user_id).await
            }
            ReferenceTarget::Foreign | ReferenceTarget::Other => {
                debug!("Moderation tag '{label}' on {uri} names nothing Nexus deletes");
                Ok(())
            }
            target @ (ReferenceTarget::Post(_)
            | ReferenceTarget::Tag { .. }
            | ReferenceTarget::File(_)) => {
                // Recorded under the address the tag names, as an event for it carries it: a
                // `pubky.app` or another app's address may spell it with a trailing slash or a
                // query, rewritten here; a `social/v1` address is canonical by spec.
                // TODO(specs-migration): a takedown through a `social/v1` address does not hold
                // against the record's `pubky.app` events before the flip, nor against a
                // `social/v1` post's version paths after it; both are settled with the flip (N3).
                let address = takedown_key(&uri).map_err(EventProcessorError::generic)?;

                // Posts, tags and files are remembered before they are deleted, so they stay
                // down even if their own event is read only later, or again, or is being
                // written right now by another loop (see `handle_put_event`): each has an
                // address of its own.
                info!("Moderation tag '{label}' on {uri} detected. Taking down {address}");
                record_takedown(&address, tag_uri).await?;
                Self::remove_taken_down(target, files_path, ingestor).await
            }
        }
    }

    /// Deletes the post, tag or file `target` names, which the moderator took down. Posts are
    /// hard-deleted when edge-free or tombstoned when engagement edges remain, as for a
    /// user's own DEL event. Deleting what isn't indexed reports
    /// [`EventProcessorError::SkipIndexing`], as a user's DEL does. A user or any other
    /// target is never taken down, so it is a no-op here.
    pub(crate) async fn remove_taken_down(
        target: ReferenceTarget,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        match target {
            ReferenceTarget::Post(key) => handlers::post::del(key, ingestor).await,
            ReferenceTarget::Tag { key } => handlers::tag::del(key).await,
            ReferenceTarget::File(key) => handlers::file::del(key, files_path).await,
            ReferenceTarget::User(_) | ReferenceTarget::Foreign | ReferenceTarget::Other => Ok(()),
        }
    }
}
