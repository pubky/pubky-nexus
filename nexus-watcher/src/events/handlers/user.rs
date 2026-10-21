use crate::events::EventProcessorError;

use nexus_common::db::queries::get::user_is_safe_to_delete;
use nexus_common::db::{exec_single_row, execute_graph_operation, queries, OperationOutcome};
use nexus_common::models::user::{UserCounts, UserDetails, UserSearch, UsersByTagSearch};
use pubky_app_specs::{PubkyAppUser, PubkyId};
use tracing::debug;

/// `uri` is the address the profile was read from: the event path.
#[tracing::instrument(name = "user.put", skip_all, fields(user_id = %user_id))]
pub async fn sync_put(
    user: PubkyAppUser,
    uri: String,
    user_id: PubkyId,
) -> Result<(), EventProcessorError> {
    debug!("Indexing user profile");

    // Step 1: Create `UserDetails` object
    let user_details = UserDetails::from_homeserver(user, &user_id);

    // Step 2: Save to graph, with the address the profile was read from
    user_details.put_profile_to_graph(&uri).await?;

    // Step 3: Index the profile: drops the previous name member and clears the
    // tombstone. Retry-safe, the previous member comes from the search state
    // itself.
    UserSearch::index_profile(&user_details).await?;

    // Step 4: If new user (no existing counts), save a new `UserCounts`
    // TODO: Use SCARD on a set for unique tag count to avoid race conditions in parallel processing
    if UserCounts::get_from_index(&user_id).await?.is_none() {
        UserCounts::default().put_to_index(&user_id).await?;
    }

    // The search write above cleared the tombstone, so re-derive the user's
    // per-label search entries afterwards: a recreated profile gets its
    // retained tags back.
    UsersByTagSearch::sync_user_labels(&user_id).await?;

    Ok(())
}

#[tracing::instrument(name = "user.del", skip_all, fields(user_id = %user_id))]
pub async fn del(user_id: PubkyId) -> Result<(), EventProcessorError> {
    debug!("Deleting user profile");

    // 1. Graph query to check if there is any edge at all to this user.
    let query = user_is_safe_to_delete(&user_id);

    // 2. If there are no relationships (OperationOutcome::CreatedOrDeleted), delete from graph and redis.
    // 3. If there are relationships (OperationOutcome::Updated), overwrite the node with a cleared
    // profile carrying `deleted = true`. The node survives so its edges stay intact.
    match execute_graph_operation(query).await? {
        OperationOutcome::CreatedOrDeleted => {
            // 1. Drop the user from search; idempotent, the member comes from the search state
            UserSearch::delete(&user_id).await?;

            // 2. Idempotent Redis cleanup
            UserCounts::delete(&user_id).await?;

            // 3. Graph deletion LAST
            exec_single_row(queries::del::delete_user(&user_id)).await?;
        }
        OperationOutcome::Updated => {
            // 1. Graph-first: the tombstone keeps the stored `uri` and its epoch.
            let tombstone = UserDetails::tombstone(&user_id);
            tombstone.put_to_graph().await?;

            // 2. Drop the user from search and mark them deleted, atomically: from
            // here on the users-by-tag sync evicts them, so a concurrent tag event
            // cannot re-add them. A retry finds the same state and changes nothing.
            UserSearch::mark_tombstoned(&user_id).await?;

            // 3. Evict the user from every per-label users-by-tag sorted set.
            UsersByTagSearch::sync_user_labels(&user_id).await?;
        }
        OperationOutcome::MissingDependency => return Err(EventProcessorError::SkipIndexing),
    }

    // TODO notifications for deleted user

    Ok(())
}
