use nexus_common::models::event::EventProcessorError;
use nexus_common::models::post::{PostDetails, PostRelationships};
use pubky_app_specs::PubkyAppPostKind;

/// Checks if a post is a reply based on its relationships.
/// # Arguments
/// * `author_id` - The ID of the author of the post
/// * `post_id` - The ID of the post to check
///
pub async fn post_relationships_is_reply(
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
