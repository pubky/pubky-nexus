use crate::db::graph::exec::exec_single_row;
use crate::events::handlers::{post::parse_post_id, user::parse_user_id};
use crate::models::post::{Bookmark, PostStream};
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppBookmark;
use crate::models::user::PubkyId;
use crate::{queries, RedisOps};
use axum::body::Bytes;
use chrono::Utc;
use log::debug;
use std::error::Error;

//TODO: only /posts/ are bookmarkable as of now.
pub async fn put(
    user_id: PubkyId,
    bookmark_id: &str,
    blob: Bytes,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Indexing new bookmark: {} -> {}", user_id, bookmark_id);

    // Deserialize and validate bookmark
    let bookmark = <PubkyAppBookmark as Validatable>::try_from(&blob)?;

    // Parse the URI to extract author_id and post_id using the updated parse_post_uri
    let (author_id, post_id) = parse_bookmarked_post_uri(&bookmark.uri)?;

    // Save new bookmark relationship to the graph
    let indexed_at = Utc::now().timestamp_millis();
    let bookmark_details = Bookmark {
        id: bookmark_id.to_string(),
        indexed_at,
    };

    let query = queries::write::create_post_bookmark(
        &user_id,
        author_id.as_ref(),
        &post_id,
        bookmark_id,
        indexed_at,
    );
    exec_single_row(query).await?;

    // Save to Redis
    bookmark_details
        .put_index_json(&[&author_id, &post_id, &user_id])
        .await?;
    PostStream::add_to_bookmarks_sorted_set(&bookmark_details, &user_id, &post_id, &author_id)
        .await?;

    Ok(())
}

pub async fn del(user_id: PubkyId, bookmark_id: &str) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting bookmark: {} -> {}", user_id, bookmark_id);

    // Delete the bookmark relationship from the graph
    let query = queries::write::delete_bookmark(&user_id, bookmark_id);
    exec_single_row(query).await?;

    // TODO DELETE FROM REDIS
    Ok(())
}

// Parses a bookmark id from the event's uri
pub fn parse_bookmark_id(uri: &str) -> Result<&str, Box<dyn std::error::Error + Send + Sync>> {
    let bookmark_segment = "/bookmarks/";
    let start_idx = uri
        .find(bookmark_segment)
        .map(|start| start + bookmark_segment.len())
        .ok_or("Bookmark segment not found in URI")?;

    Ok(&uri[start_idx..])
}

// Parse the bookmarked post URI to extract author_id and post_id
pub fn parse_bookmarked_post_uri(
    uri: &str,
) -> Result<(PubkyId, String), Box<dyn std::error::Error + Send + Sync>> {
    let author_id = parse_user_id(uri)?;
    let post_id = parse_post_id(uri)?;

    Ok((author_id, post_id))
}
