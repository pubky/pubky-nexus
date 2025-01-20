use crate::db::graph::exec::{execute_graph_operation, OperationOutcome};
use crate::events::error::EventProcessorError;
use crate::models::user::UserSearch;
use crate::models::{
    traits::Collection,
    user::{UserCounts, UserDetails},
};
use crate::queries::get::user_is_safe_to_delete;
use crate::types::DynError;
use crate::types::PubkyId;
use log::debug;
use pubky_app_specs::{traits::Validatable, PubkyAppUser};

pub async fn put(user_id: PubkyId, blob: &[u8]) -> Result<(), DynError> {
    // Process profile.json and update the databases
    debug!("Indexing new user profile: {}", user_id);

    // Serialize and validate
    let user = <PubkyAppUser as Validatable>::try_from(blob, &user_id)?;

    sync_put(user, user_id).await
}

pub async fn sync_put(user: PubkyAppUser, user_id: PubkyId) -> Result<(), DynError> {
    // Create UserDetails object
    let user_details = UserDetails::from_homeserver(user, &user_id).await?;
    // SAVE TO GRAPH
    match user_details.put_to_graph().await {
        Ok(_) => (),
        Err(_) => return Err(EventProcessorError::UserNotSync.into()),
    }
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
