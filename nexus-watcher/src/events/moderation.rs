use std::path::PathBuf;

use crate::events::handlers;
use crate::events::handlers::universal_tag::try_parse_app_tag_path;
use nexus_common::models::event::EventProcessorError;
use pubky_app_specs::{ParsedUri, PubkyAppTag, PubkyId, Resource};
use tracing::info;

pub struct Moderation {
    /// Moderator trusted user id
    pub id: PubkyId,
    /// Tags to be moderated (tagged content is deleted)
    pub tags: Vec<String>,
}

impl Moderation {
    pub async fn should_delete(&self, tag: &PubkyAppTag, tagger_id: PubkyId) -> bool {
        tagger_id == self.id && self.tags.contains(&tag.label)
    }

    #[tracing::instrument(name = "moderation.apply", skip_all)]
    pub async fn apply_moderation(
        moderator_tag: PubkyAppTag,
        files_path: PathBuf,
    ) -> Result<(), EventProcessorError> {
        // Check if the tagged URI is a universal tag (non-pubky.app tag path such as
        // pubky://<user_id>/pub/<app>/tags/<tag_id>). These are rejected by ParsedUri::try_from
        // because it only recognises pubky.app-prefixed paths, so we handle them first.
        if let Some(info) = try_parse_app_tag_path(moderator_tag.uri.as_str()) {
            info!(
                "Moderation tag '{}' detected. Deleting universal tag {}:{}",
                moderator_tag.label, info.user_id, info.tag_id
            );
            let tag_to_del = handlers::tag::TagPath::universal(info.user_id, info.app, info.tag_id);
            return handlers::tag::del(tag_to_del).await;
        }

        // Parse the embedded URI to extract author_id and post_id using parse_tagged_post_uri
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
                let tag_path = handlers::tag::TagPath::from_pubky_app_uri(&moderator_tag.uri)?
                    .ok_or_else(|| {
                        EventProcessorError::InvalidEventLine(format!(
                            "Moderation tag did not point to a tag URI: {}",
                            moderator_tag.uri
                        ))
                    })?;
                handlers::tag::del(tag_path).await
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
