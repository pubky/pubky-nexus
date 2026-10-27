use crate::errors::EventProcessorError;
use nexus_common::models::post::PostKind as PubkyAppPostKind;
use nexus_common::models::{
    error::{ModelError, ModelResult},
    post::{PostDetails, PostRelationships},
};

/// Classifies the outcome of a best-effort user ingestion attempted while
/// handling an [`OperationOutcome::MissingDependency`](nexus_common::db::OperationOutcome::MissingDependency).
///
/// Propagates the failures no retry can cure, so the event is dropped instead of
/// churning in the retry queue: [`ModelError::HsBlacklisted`] becomes the non-retryable
/// [`EventProcessorError::HsBlacklisted`], and [`ModelError::InvalidInput`], an id that is
/// no public key, becomes [`EventProcessorError::SpecValidation`]. Any other ingestion
/// error is swallowed: the handler returns `MissingDependency` anyway, and the next retry
/// re-attempts the ingestion. (`maybe_ingest_user` already logs the underlying failure.)
pub(super) fn fail_on_permanent_ingest_error(
    ingest_result: ModelResult<()>,
) -> Result<(), EventProcessorError> {
    match ingest_result {
        Err(e @ (ModelError::HsBlacklisted { .. } | ModelError::InvalidInput(_))) => Err(e.into()),
        _ => Ok(()),
    }
}

/// The most chars of a user-supplied string an error message embeds.
const MESSAGE_TEXT_MAX_CHARS: usize = 256;

/// `text` cut to [`MESSAGE_TEXT_MAX_CHARS`] on a char boundary, with `…` when cut, so a message
/// that quotes an address stays bounded whatever the author wrote.
pub(super) fn bounded(text: &str) -> String {
    match text.char_indices().nth(MESSAGE_TEXT_MAX_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
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

/// The post's kind; a missing target reads back as `Unknown`.
pub async fn post_kind(
    author_id: &str,
    post_id: &str,
) -> Result<PubkyAppPostKind, EventProcessorError> {
    Ok(PostDetails::get_by_id(author_id, post_id)
        .await?
        .map(|details| details.kind)
        .unwrap_or(PubkyAppPostKind::Unknown))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_ingest_errors_drop_the_event() {
        let blacklisted =
            fail_on_permanent_ingest_error(Err(ModelError::HsBlacklisted { hs_id: "hs".into() }));
        assert!(
            matches!(blacklisted, Err(EventProcessorError::HsBlacklisted { .. })),
            "{blacklisted:?}"
        );

        let invalid = fail_on_permanent_ingest_error(Err(ModelError::InvalidInput("id".into())));
        assert!(
            matches!(invalid, Err(EventProcessorError::SpecValidation(_))),
            "{invalid:?}"
        );
    }

    /// A failure a retry may cure leaves the handler to report its missing dependency.
    #[test]
    fn other_ingest_outcomes_are_swallowed() {
        assert!(fail_on_permanent_ingest_error(Ok(())).is_ok());
        assert!(fail_on_permanent_ingest_error(Err(ModelError::Generic("dht".into()))).is_ok());
    }

    #[test]
    fn bounded_keeps_short_text_and_cuts_long_text_on_a_char_boundary() {
        assert_eq!(bounded("pubky://short"), "pubky://short");
        let exact = "é".repeat(MESSAGE_TEXT_MAX_CHARS);
        assert_eq!(bounded(&exact), exact);
        let long = "é".repeat(MESSAGE_TEXT_MAX_CHARS + 10);
        assert_eq!(bounded(&long), format!("{exact}…"));
    }
}
