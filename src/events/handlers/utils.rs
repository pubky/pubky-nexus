use crate::{models::post::PostRelationships, types::DynError};

/// Checks if a post is a reply based on its relationships.
/// # Arguments
/// * `author_id` - The ID of the author of the post
/// * `post_id` - The ID of the post to check
///
pub async fn post_relationships_is_reply(author_id: &str, post_id: &str) -> Result<bool, DynError> {
    match PostRelationships::get_by_id(author_id, post_id).await? {
        Some(relationship) => Ok(relationship.replied.is_some()),
        // If the post does not exist, it is treated as a reply to avoid incorrect assumptions
        None => Ok(true),
    }
}

/// A macro to handle the results of `tokio::join!` by checking for errors and propagating them.
///
/// This macro takes multiple `Result<T, E>` values (such as those returned from `tokio::join!`)
/// and iterates over them. If any result is an `Err`, it maps the error into an `EventProcessorError`
/// and propagates
#[macro_export]
macro_rules! handle_join_results {
    ($($res:expr),+) => {
        {   // Convert tuple to array
            let results = [$($res),+];
            for result in results {
                result.map_err(|e| EventProcessorError::IndexWriteFailed {
                    message: e.to_string(),
                })?;
            }
        }
    };
}
