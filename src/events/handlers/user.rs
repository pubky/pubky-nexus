use crate::models::{
    pubky_app::{traits::Validatable, PubkyAppUser},
    traits::Collection,
    user::{PubkyId, UserCounts, UserDetails},
};
use crate::reindex::reindex_user;
use axum::body::Bytes;
use log::debug;
use std::error::Error;

pub async fn put(user_id: &PubkyId, blob: Bytes) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Process profile.json and update the databases
    debug!("Indexing new user profile: {}", user_id);

    // Serialize and validate
    let user = <PubkyAppUser as Validatable>::try_from(&blob).await?;

    // Create UserDetails object
    let user_details = UserDetails::from_homeserver(user, &user_id).await?;

    // Add new node into the graph
    user_details.put_to_graph().await?;

    // Reindex to sorted sets and other indexes
    reindex_user(&user_id).await?;
    UserDetails::to_index(&[user_id.as_ref()], vec![Some(user_details)]).await?;

    Ok(())
}

pub async fn del(user_id: &PubkyId) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting user profile:  {}", user_id);

    if let Some(user_details) = UserDetails::get_by_id(&user_id).await? {
        user_details.delete().await?;
        UserCounts::delete(&user_id).await?;
    }

    Ok(())
}

// Parses a user pubky id from the event's uri
pub fn parse_user_id(uri: &str) -> Result<PubkyId, Box<dyn std::error::Error + Send + Sync>> {
    // Define the patterns we are looking for in the URI
    let pattern = "pubky://";
    let pub_segment = "/pub/";

    // Find the starting position of the user_id part in the URI
    let start_idx = uri
        .find(pattern)
        .map(|start| start + pattern.len())
        .ok_or("Pattern not found in URI")?;

    // Find the ending position of the user_id part
    let end_idx = uri[start_idx..]
        .find(pub_segment)
        .ok_or("Pub segment not found in URI")?;

    // Extract the user_id and attempt to convert it into a PubkyId
    let user_id_str = &uri[start_idx..start_idx + end_idx];
    let user_id = PubkyId::try_from(user_id_str)?;

    Ok(user_id)
}
