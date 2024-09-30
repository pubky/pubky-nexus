use crate::db::graph::exec::exec_single_row;
use crate::events::uri::ParsedUri;
use crate::models::post::Bookmark;
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppBookmark;
use crate::models::user::PubkyId;
use crate::queries;
use axum::body::Bytes;
use chrono::Utc;
use log::debug;
use std::error::Error;

//TODO: only /posts/ are bookmarkable as of now.
pub async fn put(
    user_id: PubkyId,
    bookmark_id: String,
    blob: Bytes,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Indexing new bookmark: {} -> {}", user_id, bookmark_id);

    // Deserialize and validate bookmark
    let bookmark = <PubkyAppBookmark as Validatable>::try_from(&blob).await?;

    sync_put(user_id, bookmark, bookmark_id).await
}

pub async fn sync_put(
    user_id: PubkyId,
    bookmark: PubkyAppBookmark,
    bookmark_id: String,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Parse the URI to extract author_id and post_id using the updated parse_post_uri
    let parsed_uri = ParsedUri::try_from(bookmark.uri.as_str())?;
    let (author_id, post_id) = (
        parsed_uri.user_id,
        parsed_uri.post_id.ok_or("Bookmarked URI missing post_id")?,
    );

    // Save new bookmark relationship to the graph
    let indexed_at = Utc::now().timestamp_millis();
    let bookmark_details = Bookmark {
        id: bookmark_id.to_string(),
        indexed_at,
    };

    // SAVE TO GRAPH
    let query = queries::write::create_post_bookmark(
        &user_id,
        author_id.as_ref(),
        &post_id,
        &bookmark_id,
        indexed_at,
    );
    exec_single_row(query).await?;

    // SAVE TO INDEX
    bookmark_details.put_to_index(&author_id, &post_id, &user_id).await?;
    Ok(())
}

pub async fn del(
    user_id: PubkyId,
    bookmark_id: String,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting bookmark: {} -> {}", user_id, bookmark_id);

    // Delete the bookmark relationship from the graph
    let query = queries::write::delete_bookmark(&user_id, &bookmark_id);
    exec_single_row(query).await?;

    // TODO DELETE FROM REDIS
    Ok(())
}
