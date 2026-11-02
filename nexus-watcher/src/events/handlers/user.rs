use crate::events::EventProcessorError;

use nexus_common::db::queries::get::user_is_safe_to_delete;
use nexus_common::db::{
    exec_single_row, execute_graph_operation, queries, OperationOutcome, RedisOps,
};
use nexus_common::models::{
    traits::Collection,
    user::{Epoch, UserDetails, UserSearch, UsersByTagSearch},
};
use pubky_social_specs::PubkyId;
use tracing::{debug, Instrument};

use crate::events::inputs::UserInput;

#[tracing::instrument(name = "user.put", skip_all, fields(user_id = %user_id))]
pub async fn sync_put(user: UserInput, user_id: PubkyId) -> Result<(), EventProcessorError> {
    debug!("Indexing user profile");

    // Step 1: Create `UserDetails` object
    let uri = user.uri.clone();
    let mut user_details = user.into_details(&user_id);

    // Step 2: Save to graph, with the address the profile is stored under. The node keeps its
    // epoch, which the cache copies.
    user_details.put_profile_to_graph(&uri).await?;

    // Step 3: Reindex search BEFORE refreshing the details cache. `put_to_index`
    // resolves the stale `name:id` member from the cached JSON, so a concurrent
    // `UserDetails::put_to_index` could overwrite it with the new name first and
    // leak the old member forever.
    UserSearch::put_to_index(&[&user_details]).await?;

    // Step 4: Refresh the details cache: SAVE TO INDEX
    UserDetails::put_to_index(&[&user_details.id], vec![Some(user_details.clone())])
        .instrument(tracing::info_span!("index.write"))
        .await?;

    // A profile write always clears the tombstone, so re-derive the user's
    // per-label search entries afterwards: a recreated profile gets its
    // retained tags back.
    UsersByTagSearch::sync_user_labels(&user_id).await?;

    Ok(())
}

/// Moves a user to `v1`, once [`sync_put`] has indexed their `social/v1` profile: the flip.
///
/// A failure before the epoch write leaves the user at `v0`, so the retry runs the whole flip
/// again; one after it is retried as a profile edit, which caches the stored `v1`.
#[tracing::instrument(name = "user.flip", skip_all, fields(user_id = %user_id))]
pub async fn flip(user_id: &PubkyId) -> Result<(), EventProcessorError> {
    debug!("Flipping user to v1");

    // Step 1: Set the epoch in the graph
    Epoch::set_v1_in_graph(user_id).await?;

    // Step 2: Refresh the details cache from the graph, so the API shows `v1`
    let user_id: &str = user_id.as_ref();
    let user_details = UserDetails::get_from_graph(&[user_id]).await?;
    UserDetails::put_to_index(&[user_id], user_details)
        .instrument(tracing::info_span!("index.write"))
        .await?;

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
            // 1. UserSearch reads UserDetails — must run before UserDetails Redis is removed
            UserSearch::delete(&user_id).await?;

            // 2. Redis cleanup (idempotent DEL)
            let user_id_str: &str = user_id.as_ref();
            UserDetails::remove_from_index_multiple_json(&[&[user_id_str]])
                .instrument(tracing::info_span!("index.delete"))
                .await?;

            // 3. Graph deletion LAST
            exec_single_row(queries::del::delete_user(&user_id)).await?;
        }
        OperationOutcome::Updated => {
            // 1. UserSearch resolves the indexed name from UserDetails — must run
            // before the profile is wiped, or the stale entry cannot be removed.
            UserSearch::delete(&user_id).await?;

            // 2. Graph-first: write the tombstone before the cache. Collection::get_by_ids
            // repopulates the cache from the graph on a miss, so writing the cache first
            // would let a concurrent read cache the live profile again afterwards.
            // The tombstone keeps the stored `uri` and its epoch in the graph.
            let mut tombstone = UserDetails::tombstone(&user_id);
            tombstone.put_tombstone_to_graph().await?;

            // 3. Cache the tombstone rather than invalidating: the users-by-tag removal
            // guard reads `$.deleted` from this document and a missing key reads as
            // live, so a concurrent tag event would re-add the user.
            UserDetails::put_to_index(&[&tombstone.id], vec![Some(tombstone.clone())]).await?;

            // 4. Evict the user from every per-label users-by-tag sorted set.
            UsersByTagSearch::sync_user_labels(&user_id).await?;
        }
        OperationOutcome::MissingDependency => return Err(EventProcessorError::SkipIndexing),
    }

    // TODO notifications for deleted user

    Ok(())
}
