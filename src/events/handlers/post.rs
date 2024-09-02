use crate::events::Event;
use crate::models::pubky_app::traits::Validatable;
use crate::models::{post::PostDetails, pubky_app::PubkyAppPost, user::PubkyId};
use crate::reindex::reindex_post;
use axum::body::Bytes;
use log::debug;
use std::error::Error;

pub async fn put(
    author_id: PubkyId,
    post_id: String,
    blob: Bytes,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Process Post resource and update the databases
    debug!("Indexing new post: {}/{}", author_id, post_id);

    // Serialize and validate
    let post = <PubkyAppPost as Validatable>::try_from(&blob)?;

    // Create PostDetails object
    let post_details = PostDetails::from_homeserver(post, &author_id, &post_id).await?;

    // Add new post node into the graph
    post_details.put_to_graph().await?;

    // Reindex to sorted sets and other indexes
    reindex_post(&author_id, &post_id).await?;

    Ok(())
}

pub async fn del(author_id: PubkyId, post_id: String) -> Result<(), Box<dyn Error + Sync + Send>> {
    // TODO: handle deletion of Post resource from databases
    debug!("Deleting post: {}/{}", author_id, post_id);
    // Implement logic here
    Ok(())
}

// Parses a post id from the event's uri
pub fn parse_post_id(event: &Event) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Define the pattern we're looking for in the URI
    let post_segment = "/posts/";

    // Find the starting position of the post_id part in the URI
    let start_idx = event
        .uri
        .path
        .find(post_segment)
        .map(|start| start + post_segment.len())
        .ok_or("Post segment not found in URI")?;

    // Extract the post_id from the path
    let post_id = &event.uri.path[start_idx..];

    // Return the post_id as a string
    Ok(post_id.to_string())
}
