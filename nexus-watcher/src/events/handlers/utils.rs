use crate::errors::EventProcessorError;
use nexus_common::models::{
    error::{ModelError, ModelResult},
    post::{PostDetails, PostRelationships},
};
use pubky_app_specs::PubkyAppPostKind;

/// TTL (in seconds) for delete-tombstone guard keys: 6 hours.
///
/// The tombstone must outlive the event retry backoff window so that a retried
/// delete still observes the guard acquired by the first attempt. If the delete
/// dead-letters and never completes, the key expires on its own and leaves no
/// permanent garbage behind. This value must exceed the worst-case retry window
/// derived from the `EventRetryConfig` `max_retries`/`max_backoff_secs` settings
/// (roughly 2.5 hours with defaults) and should be revisited if those are raised.
pub const DELETION_GUARD_TTL_SECS: u64 = 21600;

/// Redis key of the SETNX tombstone marking an in-flight post deletion.
///
/// Unlike the `PostRelationships` index gate, this key cannot be recreated by
/// read-through cache population, so it survives the retry window even if a
/// concurrent read resurrects the index entry from the still-present graph node.
pub fn post_deletion_guard_key(author_id: &str, post_id: &str) -> String {
    format!("Deleting:Post:{author_id}:{post_id}")
}

/// Redis key of the SETNX tombstone marking an in-flight follow deletion.
///
/// Unlike the `Followers` index gate, this key cannot be recreated by
/// read-through cache population, so it survives the retry window even if a
/// concurrent read resurrects the follow sets from the still-present graph edge.
pub fn follow_deletion_guard_key(follower_id: &str, followee_id: &str) -> String {
    format!("Deleting:Follow:{follower_id}:{followee_id}")
}

/// Classifies the outcome of a best-effort user ingestion attempted while
/// handling an [`OperationOutcome::MissingDependency`](nexus_common::db::OperationOutcome::MissingDependency).
///
/// Propagates [`ModelError::HsBlacklisted`] as the non-retryable
/// [`EventProcessorError::HsBlacklisted`], so the event is dropped instead of
/// churning in the retry queue. Any other ingestion error is swallowed: the handler
/// returns `MissingDependency` anyway, and the next retry re-attempts the ingestion.
/// (`maybe_ingest_user` already logs the underlying failure.)
pub(super) fn fail_on_blacklisted_hs(
    ingest_result: ModelResult<()>,
) -> Result<(), EventProcessorError> {
    match ingest_result {
        Err(e @ ModelError::HsBlacklisted { .. }) => Err(e.into()),
        _ => Ok(()),
    }
}

/// Checks if a post is a reply based on its relationships.
/// # Arguments
/// * `author_id` - The ID of the author of the post
/// * `post_id` - The ID of the post to check
pub(super) async fn post_relationships_is_reply(
    author_id: &str,
    post_id: &str,
) -> Result<bool, EventProcessorError> {
    match PostRelationships::get_by_id(author_id, post_id).await? {
        Some(relationship) => Ok(relationship.replied.is_some()),
        // If the post does not exist, it is treated as a reply to avoid incorrect assumptions
        None => Ok(true),
    }
}

/// Whether a post is a Collection. A missing/unknown target defaults to `false`
/// (a normal post) so the increment and decrement paths stay symmetric.
pub async fn post_is_collection(
    author_id: &str,
    post_id: &str,
) -> Result<bool, EventProcessorError> {
    Ok(matches!(
        PostDetails::get_by_id(author_id, post_id).await?,
        Some(details) if details.kind == PubkyAppPostKind::Collection
    ))
}
