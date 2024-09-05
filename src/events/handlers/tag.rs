use crate::db::graph::exec::exec_single_row;
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppTag;
use crate::models::user::PubkyId;
use crate::queries;
use axum::body::Bytes;
use chrono::Utc;
use log::debug;
use std::error::Error;

use super::post::parse_post_id;
use super::user::parse_user_id;

pub async fn put(
    user_id: PubkyId,
    tag_id: &str,
    blob: Bytes,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Indexing new tag: {} -> {}", user_id, tag_id);

    // Deserialize and validate tag
    let tag = <PubkyAppTag as Validatable>::try_from(&blob)?;

    // Parse the URI to extract author_id and post_id using parse_tagged_post_uri
    let (author_id, post_id) = parse_tagged_post_uri(&tag.uri)?;

    // Save new tag relationship to the graph
    let indexed_at = Utc::now().timestamp_millis();
    let query = queries::write::create_post_tag(
        &user_id, &author_id, &post_id, tag_id, &tag.label, indexed_at,
    );
    exec_single_row(query).await?;

    // TODO: index TAG to on Redis and add to sorted sets

    Ok(())
}

pub async fn del(user_id: PubkyId, tag_id: &str) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting tag: {} -> {}", user_id, tag_id);

    // Delete the tag relationship from the graph
    let query = queries::write::delete_tag(&user_id, tag_id);
    exec_single_row(query).await?;

    Ok(())
}

// Parses a tag id from the event's uri
pub fn parse_tag_id(uri: &str) -> Result<&str, Box<dyn std::error::Error + Send + Sync>> {
    let tag_segment = "/tags/";
    let start_idx = uri
        .find(tag_segment)
        .map(|start| start + tag_segment.len())
        .ok_or("Tag segment not found in URI")?;

    Ok(&uri[start_idx..])
}

// Parse the tagged post URI to extract author_id and post_id
pub fn parse_tagged_post_uri(
    uri: &str,
) -> Result<(PubkyId, String), Box<dyn std::error::Error + Send + Sync>> {
    let author_id = parse_user_id(uri)?;
    let post_id = parse_post_id(uri)?;

    Ok((author_id, post_id))
}
