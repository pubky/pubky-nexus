use crate::events::handlers;
use nexus_common::types::DynError;
use pubky_app_specs::{ParsedUri, PubkyAppTag, PubkyId, Resource};
use tracing::debug;

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

    pub async fn apply_moderation(tag: PubkyAppTag) -> Result<(), DynError> {
        // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
        let parsed_uri = ParsedUri::try_from(tag.uri.as_str())?;
        let user_id = parsed_uri.user_id;

        match parsed_uri.resource {
            Resource::Post(post_id) => {
                // Delete the post and return the result
                debug!(
                    "Moderation tag '{}' detected. Deleting post {}:{}",
                    tag.label, user_id, post_id
                );
                handlers::post::sync_del(user_id, post_id).await
            }
            Resource::Tag(tag_id) => {
                // Delete the tag and return the result
                debug!(
                    "Moderation tag '{}' detected. Deleting tag {}:{}",
                    tag.label, user_id, tag_id
                );
                handlers::tag::del(user_id, tag_id).await
            }
            _ => Ok(()),
        }
    }
}
