use crate::models::{
    pubky_app::{traits::Validatable, PubkyAppUser},
    traits::Collection,
    user::{PubkyId, UserCounts, UserDetails},
};
use axum::body::Bytes;
use log::debug;
use std::error::Error;

pub async fn put(user_id: PubkyId, blob: Bytes) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Process profile.json and update the databases
    debug!("Indexing new user profile: {}", user_id);

    // Serialize and validate
    let user = <PubkyAppUser as Validatable>::try_from(&blob).await?;

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
    UserCounts::default().put_to_index(&user_id).await?;
    UserDetails::put_to_index(&[&user_id], vec![Some(user_details)]).await?;
    Ok(())
}

pub async fn del(user_id: PubkyId) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting user profile:  {}", user_id);

    if let Some(user_details) = UserDetails::get_by_id(&user_id).await? {
        user_details.delete().await?;
        UserCounts::delete(&user_id).await?;
    }

    Ok(())
}
