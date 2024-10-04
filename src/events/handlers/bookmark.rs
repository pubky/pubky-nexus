use crate::events::uri::ParsedUri;
use crate::models::post::Bookmark;
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppBookmark;
use crate::models::user::PubkyId;
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
    id: String,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Parse the URI to extract author_id and post_id using the updated parse_post_uri
    let parsed_uri = ParsedUri::try_from(bookmark.uri.as_str())?;
    let (author_id, post_id) = (
        parsed_uri.user_id,
        parsed_uri.post_id.ok_or("Bookmarked URI missing post_id")?,
    );

    // Save new bookmark relationship to the graph
    let indexed_at = Utc::now().timestamp_millis();
    Bookmark::put_to_graph(&author_id, &post_id, &user_id, &id, indexed_at).await?;

    // SAVE TO INDEX
    let bookmark_details = Bookmark { id, indexed_at };
    bookmark_details
        .put_to_index(&author_id, &post_id, &user_id)
        .await?;
    Ok(())
}

pub async fn del(
    user_id: PubkyId,
    bookmark_id: String,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting bookmark: {} -> {}", user_id, bookmark_id);
    sync_del(user_id, bookmark_id).await
}

pub async fn sync_del(
    user_id: PubkyId,
    bookmark_id: String,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // DELETE FROM GRAPH
    let deleted_bookmark_info = Bookmark::del_from_graph(&user_id, &bookmark_id).await?;

    if deleted_bookmark_info.is_some() {
        let (post_id, author_id) = deleted_bookmark_info.unwrap();
        // DELETE FROM INDEXes
        Bookmark::del_from_index(&user_id, &post_id, &author_id).await?;
    }

    Ok(())
}
