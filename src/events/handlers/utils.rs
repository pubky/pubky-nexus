use log::debug;
use pubky_app_specs::PubkyAppUser;

use crate::events::handlers::user;
use crate::models::{post::PostRelationships, traits::Collection, user::UserDetails};
use crate::types::{DynError, PubkyId};

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

/// # Description
/// This function verifies if a user, identified by their `PubkyId`, exists in the graph database.
/// If the user is not found, it creates a "shadow user."
///
/// ## Shadow User Definition
/// A shadow user represents a Pubky key that exists in the homeserver but has not been indexed in the nexus.
/// This situation typically occurs when the `profile.json` file is missing from the homeserver.
///
/// # Parameters
/// - `user_id`: The `PubkyId` of the user to check
///
/// # Notes
/// - This operation assumes the user might not exist in the nexus and ensures their presence.
pub async fn ensure_user_indexed(user_id: PubkyId) -> Result<(), DynError> {
    let user_exists = UserDetails::get_from_graph(&[&user_id]).await?;

    if user_exists.get(0).unwrap().is_none() {
        debug!("ShadowUser: User is not indexed, {:?}", user_id);
        user::sync_put(PubkyAppUser::default(), user_id, true).await?;
        debug!("ShadowUser index successful");
    }
    Ok(())
}