use crate::events::EventProcessorError;

use nexus_common::db::queries::get::user_is_safe_to_delete;
use nexus_common::db::{
    exec_single_row, execute_graph_operation, queries, OperationOutcome, RedisOps,
};
use nexus_common::models::{
    traits::Collection,
    user::{UserCounts, UserDetails, UserSearch, UsersByTagSearch},
};
use pubky_app_specs::{PubkyAppUser, PubkyId};
use tracing::debug;

#[tracing::instrument(name = "user.put", skip_all, fields(user_id = %user_id))]
pub async fn sync_put(user: PubkyAppUser, user_id: PubkyId) -> Result<(), EventProcessorError> {
    debug!("Indexing user profile");

    // Step 1: Create `UserDetails` object
    let user_details = UserDetails::from_homeserver(user, &user_id);

    // Step 2: Save to graph
    user_details.put_to_graph().await?;

    // Step 3: Reindex search BEFORE refreshing the details cache. `put_to_index`
    // resolves the stale `name:id` member from the cached JSON, so a concurrent
    // `UserDetails::put_to_index` could overwrite it with the new name first and
    // leak the old member forever.
    UserSearch::put_to_index(&[&user_details]).await?;

    // Step 4: Run in parallel the remaining cache writes: SAVE TO INDEX
    let indexing_results = nexus_common::traced_join!(
        tracing::info_span!("index.write");
        async {
            // TODO: Use SCARD on a set for unique tag count to avoid race conditions in parallel processing
            // If new user (no existing counts), save a new `UserCounts`
            if UserCounts::get_from_index(&user_id).await?.is_none() {
                UserCounts::default().put_to_index(&user_id).await?;
            }
            Ok::<(), EventProcessorError>(())
        },
        async {
            UserDetails::put_to_index(&[&user_details.id], vec![Some(user_details.clone())])
                .await?;
            Ok::<(), EventProcessorError>(())
        }
    );

    indexing_results.0?;
    indexing_results.1?;

    // A profile write always clears the tombstone, so re-derive the user's
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
            // 1. UserSearch reads UserDetails — must run before UserDetails Redis is removed
            UserSearch::delete(&user_id).await?;

            // 2. Redis cleanup (parallel, all idempotent DEL/ZREM)
            let user_id_str: &str = user_id.as_ref();
            let key_parts: &[&str] = &[user_id_str];
            let key_parts_list = [key_parts];
            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.delete");
                UserDetails::remove_from_index_multiple_json(&key_parts_list),
                UserCounts::delete(&user_id)
            );
            indexing_results.0?;
            indexing_results.1?;

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
            let tombstone = UserDetails::tombstone(&user_id);
            tombstone.put_to_graph().await?;

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
