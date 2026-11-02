use std::path::Path;
use std::sync::Arc;

use crate::errors::EventProcessorError;
use crate::events::handlers;
use nexus_common::models::takedown::record_takedown;
use nexus_common::models::user::UserIngestor;
use nexus_common::WatcherConfig;
use pubky_app_specs::{ExtendedParsedUri, ParsedUri, PubkyAppTag, PubkyId, Resource};
use tracing::{info, warn};

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
    /// tombstoned when engagement edges remain. `tag_uri` is the moderator tag's own
    /// address, kept with the takedown.
    #[tracing::instrument(name = "moderation.apply", skip_all)]
    pub async fn apply_moderation(
        &self,
        moderator_tag: PubkyAppTag,
        tag_uri: &str,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        let lahel = moderator_tag.label;

        // `handle_error` drops a non-retryable error at debug level, so a moderator action
        // that cannot apply is logged here, where an operator will see it.
        let target = moderation_target(&moderator_tag.uri)
            .inspect_err(|e| warn!("Moderation tag '{lahel}' not applied: {e}"))?;
        let uri = match target {
            // A profile is not remembered: its address is the user's, and a moderated user
            // may come back by writing a new profile.
            ModerationTarget::User(user_id) => {
                info!("Moderation tag '{lahel}' detected. Deleting user profile {user_id}");
                return handlers::user::del(user_id).await;
            }
            ModerationTarget::Ignore => return Ok(()),
            ModerationTarget::TakeDown(uri) => uri,
        };

        // Posts, tags and files are remembered before they are deleted, so they stay down
        // even if their own event is read only later, or again, or is being written right
        // now by another loop (see `handle_put_event`): each has an address of its own.
        info!("Moderation tag '{lahel}' detected. Taking down {uri}");
        record_takedown(&uri, tag_uri).await?;
        Self::remove_taken_down(&uri, files_path, ingestor).await
    }

    /// Deletes the post, tag or file at `uri`, which the moderator took down. Posts are
    /// hard-deleted when edge-free or tombstoned when engagement edges remain, as for a
    /// user's own DEL event. Deleting what isn't indexed reports
    /// [`EventProcessorError::SkipIndexing`], as a user's DEL does.
    pub(crate) async fn remove_taken_down(
        uri: &str,
        files_path: &Path,
        ingestor: &UserIngestor,
    ) -> Result<(), EventProcessorError> {
        // ParsedUri does not handle app-specific tag storage paths (Universal Tags), so they
        // must be intercepted first.
        if handlers::tag::is_tag_storage_uri(uri) {
            handlers::tag::del(uri).await
        } else {
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
}

/// What a moderator tag acts on.
#[derive(Debug, PartialEq)]
enum ModerationTarget {
    /// A profile: the user is deleted.
    User(PubkyId),
    /// A post, tag or file, under the address an event for it carries: the tag's URI may
    /// spell it differently (a trailing slash, a query) and still name the same record.
    TakeDown(String),
    /// A pubky.app resource moderation does not act on (a follow, a bookmark, ...).
    Ignore,
}

/// Classifies the target URI of a moderator tag.
///
/// A URI that is neither a pubky.app URI nor a universal tag path (another scheme, another
/// app's non-tag path, an invalid user id) can never be taken down by retrying, so it is
/// rejected with [`EventProcessorError::UnsupportedTarget`], which is not retried, as
/// `tag::tag_target` does for an ordinary tag.
fn moderation_target(uri: &str) -> Result<ModerationTarget, EventProcessorError> {
    let unsupported = |e: String| {
        EventProcessorError::UnsupportedTarget(format!(
            "The moderated URI is not a pubky.app URI or a universal tag: {e}"
        ))
    };

    if !handlers::tag::is_tag_storage_uri(uri) {
        let parsed_uri = ParsedUri::try_from(uri).map_err(unsupported)?;
        match parsed_uri.resource {
            Resource::User => return Ok(ModerationTarget::User(parsed_uri.user_id)),
            Resource::Post(_) | Resource::Tag(_) | Resource::File(_) => {}
            _ => return Ok(ModerationTarget::Ignore),
        }
    }

    ExtendedParsedUri::try_from(uri)
        .and_then(|parsed| parsed.try_to_uri_str())
        .map(ModerationTarget::TakeDown)
        .map_err(unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::retry::RetryScheduler;

    const USER_ID: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";

    #[test]
    fn moderation_target_rejects_unsupported_targets_without_retry() {
        let cases = [
            "https://example.com/album/123".to_string(),
            format!("pubky://{USER_ID}/pub/mapky/places/1"),
            "pubky://notakey/pub/pubky.app/profile.json".to_string(),
        ];

        for uri in cases {
            let err = moderation_target(&uri).expect_err(&format!("{uri} must be rejected"));
            assert!(
                matches!(err, EventProcessorError::UnsupportedTarget(_)),
                "{uri}: unexpected error {err:?}"
            );
            assert!(
                !RetryScheduler::should_enqueue_related_event(&err),
                "{uri}: a deterministic rejection must not be retried"
            );
        }
    }

    #[test]
    fn moderation_target_classifies_supported_targets() {
        let user_id = PubkyId::try_from(USER_ID).unwrap();
        let post = format!("pubky://{USER_ID}/pub/pubky.app/posts/0033SSE3B1FQ0");
        let tag = format!("pubky://{USER_ID}/pub/pubky.app/tags/ABCDEFGHJK");
        let universal_tag = format!("pubky://{USER_ID}/pub/mapky/tags/ABCDEFGHJK");
        let file = format!("pubky://{USER_ID}/pub/pubky.app/files/0033SSE3B1FQ0");

        assert_eq!(
            moderation_target(&format!("pubky://{USER_ID}/pub/pubky.app/profile.json")).unwrap(),
            ModerationTarget::User(user_id.clone())
        );
        for uri in [&post, &tag, &universal_tag, &file] {
            assert_eq!(
                moderation_target(uri).unwrap(),
                ModerationTarget::TakeDown(uri.clone()),
                "{uri}"
            );
        }
        // A non-canonical spelling of the same record maps to the address its event carries.
        for (spelling, canonical) in [
            (format!("{post}/"), &post),
            (format!("{universal_tag}?ref=1"), &universal_tag),
        ] {
            assert_eq!(
                moderation_target(&spelling).unwrap(),
                ModerationTarget::TakeDown(canonical.clone()),
                "{spelling}"
            );
        }
        assert_eq!(
            moderation_target(&format!(
                "pubky://{USER_ID}/pub/pubky.app/follows/{USER_ID}"
            ))
            .unwrap(),
            ModerationTarget::Ignore
        );
    }

    /// A rejected moderator tag is dropped at debug level by `handle_error`, so the warn
    /// line here is the only trace a dropped takedown leaves on the live path.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn apply_moderation_rejects_and_warns_on_unsupported_target() {
        let label = "label_to_moderate".to_string();
        let moderation = Moderation {
            id: PubkyId::try_from(USER_ID).unwrap(),
            tags: vec![label.clone()],
        };
        let tag = PubkyAppTag {
            uri: "https://example.com/album/123".to_string(),
            label,
            created_at: 0,
        };
        let tag_uri = format!("pubky://{USER_ID}/pub/pubky.app/tags/ABCDEFGHJK");

        let result = moderation
            .apply_moderation(
                tag,
                &tag_uri,
                Path::new("/nonexistent"),
                &nexus_common::utils::test_utils::default_ingestor_tests(),
            )
            .await;

        assert!(
            matches!(result, Err(EventProcessorError::UnsupportedTarget(_))),
            "unexpected result {result:?}"
        );
        assert!(logs_contain("not applied"));
    }
}
