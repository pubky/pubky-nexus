use std::path::Path;
use std::sync::Arc;

use crate::errors::EventProcessorError;
use crate::events::handlers;
use nexus_common::models::user::UserIngestor;
use nexus_common::WatcherConfig;
use pubky_social_specs::legacy_v0::{ParsedUri, PubkyId, Resource};
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
    /// Returns `true` if the tag was applied by the moderator and its `label`
    /// is a moderated tag label.
    pub fn should_delete(&self, label: &str, tagger_id: &PubkyId) -> bool {
        tagger_id == &self.id && self.tags.iter().any(|tag| tag == label)
    }

    /// Apply moderation to the resource a moderator tag points at.
    ///
    /// `label` is the moderator tag's label and `moderated_uri` the URI it
    /// tags. Removes the corresponding resource (tag, user, or file). Posts are
    /// hard-deleted when edge-free or tombstoned when engagement edges remain.
    #[tracing::instrument(name = "moderation.apply", skip_all)]
    pub async fn apply_moderation(
        &self,
        label: &str,
        moderated_uri: &str,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        // ParsedUri does not handle app-specific tag storage paths (Universal Tags), so they must be intercepted first.
        if handlers::tag::is_tag_storage_uri(moderated_uri) {
            info!("Moderation tag '{label}' detected. Deleting moderated tag {moderated_uri}",);
            return handlers::tag::del(moderated_uri).await;
        }

        // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
        let parsed_uri =
            ParsedUri::try_from(moderated_uri).map_err(EventProcessorError::generic)?;
        let user_id = parsed_uri.user_id;

        match parsed_uri.resource {
            Resource::Post(post_id) => {
                info!("Moderation tag '{label}' detected. Deleting post {user_id}:{post_id}");
                // Same gate as user-issued DEL events: hard-delete when edge-free, tombstone
                // when any engagement edge remains (tags, bookmarks, replies, reposts, mentions).
                handlers::post::del(user_id, post_id, ingestor).await
            }
            Resource::Tag(tag_id) => {
                info!("Moderation tag '{label}' detected. Deleting tag {user_id}:{tag_id}");
                handlers::tag::del(moderated_uri).await
            }
            Resource::User => {
                info!("Moderation tag '{label}' detected. Deleting user profile {user_id}");
                handlers::user::del(user_id).await
            }
            Resource::File(file_id) => {
                info!("Moderation tag '{label}' detected. Deleting file {user_id}:{file_id}");
                handlers::file::del(&user_id, file_id, files_path).await
            }
            _ => Ok(()),
        }
    }
}
