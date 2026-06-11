use nexus_common::models::error::{ModelError, ModelResult};
use nexus_common::models::event::EventProcessorError;
use nexus_common::models::post::PostRelationships;

/// Classifies the outcome of a best-effort user ingestion attempted while
/// handling an [`OperationOutcome::MissingDependency`](nexus_common::db::OperationOutcome::MissingDependency).
///
/// Propagates [`ModelError::HomeserverBlacklisted`] as the non-retryable
/// [`EventProcessorError::HsBlacklisted`], so the event is dropped instead of
/// churning in the retry queue. Any other ingestion error is swallowed: the handler
/// returns `MissingDependency` anyway, and the next retry re-attempts the ingestion.
/// (`maybe_ingest_user` already logs the underlying failure.)
pub(super) fn fail_on_blacklisted_hs(
    ingest_result: ModelResult<()>,
) -> Result<(), EventProcessorError> {
    match ingest_result {
        Err(e @ ModelError::HomeserverBlacklisted { .. }) => Err(e.into()),
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
