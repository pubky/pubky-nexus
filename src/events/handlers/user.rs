use crate::db::graph::exec::exec_existed_row;
use crate::models::user::UserSearch;
use crate::models::{
    pubky_app::{traits::Validatable, PubkyAppUser},
    traits::Collection,
    user::{PubkyId, UserCounts, UserDetails},
};
use crate::queries::get::user_has_relationships;
use axum::body::Bytes;
use log::debug;
use std::error::Error;

pub async fn put(user_id: PubkyId, blob: Bytes) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Process profile.json and update the databases
    debug!("Indexing new user profile: {}", user_id);

    // Serialize and validate
    let user = <PubkyAppUser as Validatable>::try_from(&blob, &user_id).await?;

    sync_put(user, user_id).await
}

pub async fn sync_put(
    user: PubkyAppUser,
    user_id: PubkyId,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Create UserDetails object
    let user_details = UserDetails::from_homeserver(user, &user_id).await?;
    // SAVE TO GRAPH
    user_details.put_to_graph().await?;
    // SAVE TO INDEX
    let user_id = user_details.id.clone();
    UserSearch::put_to_index(&[&user_details]).await?;
    // If new user (no existing counts) save a new UserCounts.
    match UserCounts::get_from_index(&user_id).await? {
        None => UserCounts::default().put_to_index(&user_id).await?,
        Some(_) => (),
    };
    UserDetails::put_to_index(&[&user_id], vec![Some(user_details)]).await?;
    Ok(())
}

pub async fn del(user_id: PubkyId) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting user profile:  {}", user_id);

    let query = user_has_relationships(&user_id);
    let delete_safe = !exec_existed_row(query).await?; // No existing relationships for this user

    // 1. Graph query to check if there is any edge at all to this user.
    // 2. If there is no relationships, delete from graph and redis.
    // 3. But if there is any relationship, then we simply update the user with empty profile
    // and keyword username [DELETED]. A deleted user is a user whose profile is empty and has username `"[DELETED]"`
    match delete_safe {
        true => {
            UserDetails::delete(&user_id).await?;
            UserCounts::delete(&user_id).await?;
        }
        false => {
            let deleted_user = PubkyAppUser {
                name: "[DELETED]".to_string(),
                bio: None,
                status: None,
                links: None,
                image: None,
            };

            sync_put(deleted_user, user_id).await?;
        }
    }

    // TODO notifications for deleted user

    Ok(())
}
