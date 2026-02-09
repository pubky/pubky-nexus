use crate::events::EventProcessorError;

use nexus_common::db::queries::get::user_is_safe_to_delete;
use nexus_common::db::{execute_graph_operation, OperationOutcome};
use nexus_common::models::user::UserSearch;
use nexus_common::models::{
    traits::Collection,
    user::{UserCounts, UserDetails},
};
use pubky_app_specs::{PubkyAppUser, PubkyId};
use tracing::debug;

pub async fn sync_put(user: PubkyAppUser, user_id: PubkyId) -> Result<(), EventProcessorError> {
    debug!("Indexing new user profile: {}", user_id);

    // Step 1: Create `UserDetails` object
    let user_details = UserDetails::from_homeserver(user, &user_id).await?;

    // Step 2: Save to graph
    user_details
        .put_to_graph()
        .await
        .map_err(|e| EventProcessorError::GraphQueryFailed {
            message: format!("{e:?}"),
        })?;

    // Step 3: Run in parallel the cache process: SAVE TO INDEX
    let indexing_results = tokio::join!(
        async {
            UserSearch::put_to_index(&[&user_details]).await?;
            Ok::<(), EventProcessorError>(())
        },
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
    indexing_results.2?;
    Ok(())
}

pub async fn del(user_id: PubkyId) -> Result<(), EventProcessorError> {
    debug!("Deleting user profile:  {}", user_id);

    // 1. Graph query to check if there is any edge at all to this user.
    let query = user_is_safe_to_delete(&user_id);

    // 2. If there is no relationships (OperationOutcome::CreatedOrDeleted), delete from graph and redis.
    // 3. But if there is any relationship (OperationOutcome::Updated), then we simply update the user with empty profile
    // and keyword username [DELETED].
    // A deleted user is a user whose profile is empty and has username `"[DELETED]"`
    match execute_graph_operation(query).await? {
        OperationOutcome::CreatedOrDeleted => {
            let indexing_results =
                tokio::join!(UserDetails::delete(&user_id), UserCounts::delete(&user_id));
            indexing_results.0.map_err(EventProcessorError::index_write_failed)?;
            indexing_results.1.map_err(EventProcessorError::index_write_failed)?;
        }
        OperationOutcome::Updated => {
            let deleted_user = PubkyAppUser {
                name: "[DELETED]".to_string(),
                bio: None,
                status: None,
                links: None,
                image: None,
            };

            sync_put(deleted_user, user_id).await?;
        }
        OperationOutcome::MissingDependency => return Err(EventProcessorError::SkipIndexing),
    }

    // TODO notifications for deleted user

    Ok(())
}
