use crate::db::graph::exec::OperationOutcome;
use crate::db::kv::index::json::JsonAction;
use crate::events::error::EventProcessorError;
use crate::events::uri::ParsedUri;
use crate::models::post::Bookmark;
use crate::models::user::UserCounts;
use crate::types::DynError;
use chrono::Utc;
use log::debug;
use pubky_app_specs::traits::Validatable;
use pubky_app_specs::{PubkyAppBookmark, PubkyId};

//TODO: only /posts/ are bookmarkable as of now.
pub async fn put(user_id: PubkyId, bookmark_id: String, blob: &[u8]) -> Result<(), DynError> {
    debug!("Indexing new bookmark: {} -> {}", user_id, bookmark_id);

    // Deserialize and validate bookmark
    let bookmark = <PubkyAppBookmark as Validatable>::try_from(blob, &bookmark_id)?;

    sync_put(user_id, bookmark, bookmark_id).await
}

pub async fn sync_put(
    user_id: PubkyId,
    bookmark: PubkyAppBookmark,
    id: String,
) -> Result<(), DynError> {
    // Parse the URI to extract author_id and post_id using the updated parse_post_uri
    let parsed_uri = ParsedUri::try_from(bookmark.uri.as_str())?;
    let (author_id, post_id) = (
        parsed_uri.user_id,
        parsed_uri.post_id.ok_or("Bookmarked URI missing post_id")?,
    );
    // Save new bookmark relationship to the graph, only if the bookmarked user exists
    let indexed_at = Utc::now().timestamp_millis();
    let existed =
        match Bookmark::put_to_graph(&author_id, &post_id, &user_id, &id, indexed_at).await? {
            OperationOutcome::CreatedOrDeleted => false,
            OperationOutcome::Updated => true,
            OperationOutcome::MissingDependency => {
                let dependency = vec![format!("{author_id}:posts:{post_id}")];
                return Err(EventProcessorError::MissingDependency { dependency }.into());
            }
        };

    // SAVE TO INDEX
    let bookmark_details = Bookmark { id, indexed_at };
    bookmark_details
        .put_to_index(&author_id, &post_id, &user_id)
        .await?;

    // Update user counts with the new bookmark. Skip if bookmark existed.
    if !existed {
        UserCounts::update(&user_id, "bookmarks", JsonAction::Increment(1)).await?;
    }

    Ok(())
}

pub async fn del(user_id: PubkyId, bookmark_id: String) -> Result<(), DynError> {
    debug!("Deleting bookmark: {} -> {}", user_id, bookmark_id);
    sync_del(user_id, bookmark_id).await
}

pub async fn sync_del(user_id: PubkyId, bookmark_id: String) -> Result<(), DynError> {
    let deleted_bookmark_info = Bookmark::del_from_graph(&user_id, &bookmark_id).await?;
    // Ensure the bookmark exists in the graph before proceeding
    let (post_id, author_id) = match deleted_bookmark_info {
        Some(info) => info,
        None => return Err(EventProcessorError::SkipIndexing.into()),
    };

    Bookmark::del_from_index(&user_id, &post_id, &author_id).await?;
    // Update user counts
    UserCounts::update(&user_id, "bookmarks", JsonAction::Decrement(1)).await?;

    Ok(())
}
