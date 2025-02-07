use crate::db::graph::exec::{execute_graph_operation, OperationOutcome};
use crate::events::error::EventProcessorError;
use crate::models::user::UserSearch;
use crate::models::{
    traits::Collection,
    user::{UserCounts, UserDetails},
};
use crate::queries::get::user_is_safe_to_delete;
use crate::types::DynError;
use log::debug;
use pubky_app_specs::{PubkyAppUser, PubkyId};

pub async fn sync_put(user: PubkyAppUser, user_id: PubkyId) -> Result<(), DynError> {
    debug!("Indexing new user profile: {}", user_id);
    // Create UserDetails object
    let user_details = UserDetails::from_homeserver(user, &user_id).await?;
    user_details
        .put_to_graph()
        .await
        .map_err(|e| EventProcessorError::GraphQueryFailed {
            message: format!("{:?}", e),
        })?;
    // SAVE TO INDEX
    let user_id = user_details.id.clone();
    UserSearch::put_to_index(&[&user_details]).await?;
    // If new user (no existing counts) save a new UserCounts.
    if UserCounts::get_from_index(&user_id).await?.is_none() {
        UserCounts::default().put_to_index(&user_id).await?
    };
    UserDetails::put_to_index(&[&user_id], vec![Some(user_details)]).await?;
    Ok(())
}

pub async fn del(user_id: PubkyId) -> Result<(), DynError> {
    debug!("Deleting user profile:  {}", user_id);

    // 1. Graph query to check if there is any edge at all to this user.
    let query = user_is_safe_to_delete(&user_id);

    // 2. If there is no relationships (OperationOutcome::CreatedOrDeleted), delete from graph and redis.
    // 3. But if there is any relationship (OperationOutcome::Updated), then we simply update the user with empty profile
    // and keyword username [DELETED].
    // A deleted user is a user whose profile is empty and has username `"[DELETED]"`
    match execute_graph_operation(query).await? {
        OperationOutcome::CreatedOrDeleted => {
            UserDetails::delete(&user_id).await?;
            UserCounts::delete(&user_id).await?;
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
        OperationOutcome::MissingDependency => return Err(EventProcessorError::SkipIndexing.into()),
    }

    // TODO notifications for deleted user

    Ok(())
}
