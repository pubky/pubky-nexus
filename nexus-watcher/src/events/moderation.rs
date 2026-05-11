use std::path::PathBuf;

use crate::events::handlers;
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
        let lahel = moderator_tag.label;
        let moderated_uri = &moderator_tag.uri;

        // ParsedUri does not handle app-specific tag storage paths (Universal Tags), so they must be intercepted first.
        if handlers::tag::is_tag_storage_uri(&moderator_tag.uri) {
            info!("Moderation tag '{lahel}' detected. Deleting moderated tag {moderated_uri}",);
            return handlers::tag::del(moderated_uri).await;
        }

        // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
        let parsed_uri = ParsedUri::try_from(moderator_tag.uri.as_str())
            .map_err(EventProcessorError::generic)?;
        let user_id = parsed_uri.user_id;

        match parsed_uri.resource {
            Resource::Post(post_id) => {
                info!("Moderation tag '{lahel}' detected. Deleting post {user_id}:{post_id}");
                handlers::post::sync_del(user_id, post_id).await
            }
            Resource::Tag(tag_id) => {
                info!("Moderation tag '{lahel}' detected. Deleting tag {user_id}:{tag_id}");
                handlers::tag::del(&moderator_tag.uri).await
            }
            Resource::User => {
                info!("Moderation tag '{lahel}' detected. Deleting user profile {user_id}");
                handlers::user::del(user_id).await
            }
            Resource::File(file_id) => {
                info!("Moderation tag '{lahel}' detected. Deleting file {user_id}:{file_id}");
                handlers::file::del(&user_id, file_id, files_path).await
            }
            _ => Ok(()),
        }
    }
}
