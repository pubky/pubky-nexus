use std::path::PathBuf;
use std::sync::Arc;

use crate::events::handlers;
use nexus_common::models::event::EventProcessorError;
use nexus_common::WatcherConfig;
use pubky_app_specs::{ParsedUri, PubkyAppTag, PubkyId, Resource};
use tracing::info;

/// Trait for moderation operations.
///
/// This trait abstracts moderation logic to allow for flexible implementations,
/// including mocked versions for testing.
#[async_trait::async_trait]
pub trait TModeration: Send + Sync {
    /// Check if a tag should trigger deletion of the tagged content.
    ///
    /// Returns `true` if the tag was applied by the moderator and matches
    /// a moderated tag label.
    async fn should_delete(&self, tag: &PubkyAppTag, tagger_id: PubkyId) -> bool;

    /// Apply moderation by deleting the tagged resource.
    ///
    /// Parses the embedded URI in the moderator tag and deletes the corresponding
    /// resource (post, tag, user, or file).
    async fn apply_moderation(
        &self,
        moderator_tag: PubkyAppTag,
        files_path: PathBuf,
    ) -> Result<(), EventProcessorError>;
}

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
}

#[async_trait::async_trait]
impl TModeration for Moderation {
    async fn should_delete(&self, tag: &PubkyAppTag, tagger_id: PubkyId) -> bool {
        tagger_id == self.id && self.tags.contains(&tag.label)
    }

    #[tracing::instrument(name = "moderation.apply", skip_all)]
    async fn apply_moderation(
        &self,
        moderator_tag: PubkyAppTag,
        files_path: PathBuf,
    ) -> Result<(), EventProcessorError> {
        // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
        let parsed_uri = ParsedUri::try_from(moderator_tag.uri.as_str())
            .map_err(EventProcessorError::generic)?;
        let user_id = parsed_uri.user_id;

        match parsed_uri.resource {
            Resource::Post(post_id) => {
                // Delete the post and return the result
                info!(
                    "Moderation tag '{}' detected. Deleting post {}:{}",
                    moderator_tag.label, user_id, post_id
                );
                handlers::post::sync_del(user_id, post_id).await
            }
            Resource::Tag(tag_id) => {
                // Delete the tag and return the result
                info!(
                    "Moderation tag '{}' detected. Deleting tag {}:{}",
                    moderator_tag.label, user_id, tag_id
                );
                handlers::tag::del(user_id, tag_id).await
            }
            Resource::User => {
                // Delete the user profile and return the result
                info!(
                    "Moderation tag '{}' detected. Deleting user profile {}",
                    moderator_tag.label, user_id
                );
                handlers::user::del(user_id).await
            }
            Resource::File(file_id) => {
                // Delete the file and return the result
                info!(
                    "Moderation tag '{}' detected. Deleting file {}:{}",
                    moderator_tag.label, user_id, file_id
                );
                handlers::file::del(&user_id, file_id, files_path).await
            }
            _ => Ok(()),
        }
    }
}
