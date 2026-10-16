use std::path::Path;
use std::sync::Arc;

use crate::errors::EventProcessorError;
use crate::events::handlers;
use nexus_common::models::takedown::record_takedown;
use nexus_common::models::user::UserIngestor;
use nexus_common::WatcherConfig;
use pubky_app_specs::{ParsedUri, PubkyAppTag, PubkyId, Resource};
use tracing::info;

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
    /// Returns `true` if the tag was applied by the moderator and matches
    /// a moderated tag label.
    pub fn should_delete(&self, tag: &PubkyAppTag, tagger_id: &PubkyId) -> bool {
        tagger_id == &self.id && self.tags.contains(&tag.label)
    }

    /// Apply moderation to the tagged resource.
    ///
    /// Parses the embedded URI in the moderator tag and removes the corresponding
    /// resource (tag, user, or file). Posts are hard-deleted when edge-free or
    /// tombstoned when engagement edges remain.
    #[tracing::instrument(name = "moderation.apply", skip_all)]
    pub async fn apply_moderation(
        &self,
        moderator_tag: PubkyAppTag,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        let lahel = moderator_tag.label;
        let moderated_uri = &moderator_tag.uri;

        // A profile is not remembered: its address is the user's, and a moderated user may
        // come back by writing a new profile.
        if !handlers::tag::is_tag_storage_uri(moderated_uri) {
            let parsed_uri = ParsedUri::try_from(moderated_uri.as_str())
                .map_err(EventProcessorError::generic)?;
            match parsed_uri.resource {
                Resource::User => {
                    let user_id = parsed_uri.user_id;
                    info!("Moderation tag '{lahel}' detected. Deleting user profile {user_id}");
                    return handlers::user::del(user_id).await;
                }
                Resource::Post(_) | Resource::Tag(_) | Resource::File(_) => {}
                _ => return Ok(()),
            }
        }

        // Posts, tags and files are remembered before they are deleted, so they stay down
        // even if their own event is read only later, or again, or is being written right
        // now by another loop (see `handle_put_event`): each has an address of its own.
        info!("Moderation tag '{lahel}' detected. Taking down {moderated_uri}");
        record_takedown(moderated_uri).await?;
        Self::remove_taken_down(moderated_uri, files_path, ingestor).await
    }

    /// Deletes the post, tag or file at `uri`, which the moderator took down. Posts are
    /// hard-deleted when edge-free or tombstoned when engagement edges remain, as for a
    /// user's own DEL event.
    pub(crate) async fn remove_taken_down(
        uri: &str,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        // ParsedUri does not handle app-specific tag storage paths (Universal Tags), so they
        // must be intercepted first.
        if handlers::tag::is_tag_storage_uri(uri) {
            return handlers::tag::del(uri).await;
        }
        let parsed_uri = ParsedUri::try_from(uri).map_err(EventProcessorError::generic)?;
        let user_id = parsed_uri.user_id;
        match parsed_uri.resource {
            Resource::Post(post_id) => handlers::post::del(user_id, post_id, ingestor).await,
            Resource::Tag(_) => handlers::tag::del(uri).await,
            Resource::File(file_id) => handlers::file::del(&user_id, file_id, files_path).await,
            _ => Ok(()),
        }
    }
}
