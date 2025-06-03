use std::path::PathBuf;

use crate::events::handlers;
use nexus_common::types::DynError;
use pubky_app_specs::{ParsedUri, PubkyAppTag, PubkyId, Resource};
use tracing::info;

pub struct Moderation {
    // Moderator trusted user id
    pub id: PubkyId,
    // Tags to be moderated (tagged content is deleted)
    pub tags: Vec<String>,
}

impl Moderation {
    pub async fn should_delete(&self, tag: &PubkyAppTag, tagger_id: PubkyId) -> bool {
        tagger_id == self.id && self.tags.contains(&tag.label)
    }

    pub async fn apply_moderation(
        moderator_tag: PubkyAppTag,
        files_path: PathBuf,
    ) -> Result<(), DynError> {
        // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
        let parsed_uri = ParsedUri::try_from(moderator_tag.uri.as_str())?;
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
