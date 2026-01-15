use chrono::Utc;
use nexus_common::db::OperationOutcome;
use nexus_common::models::post::Bookmark;
use nexus_common::models::user::UserCounts;
use nexus_common::types::DynError;
use pubky_app_specs::{ParsedUri, PubkyAppBookmark, PubkyId, Resource};
use tracing::debug;

use crate::events::EventProcessorError;

pub async fn sync_put(
    user_id: PubkyId,
    bookmark: PubkyAppBookmark,
    id: String,
) -> Result<(), DynError> {
    debug!("Indexing new bookmark: {} -> {}", user_id, id);
    // Parse the URI to extract author_id and post_id using the updated parse_post_uri
    let parsed_uri = ParsedUri::try_from(bookmark.uri.as_str())?;
    let author_id = parsed_uri.user_id;
    let post_id = match parsed_uri.resource {
        Resource::Post(id) => id,
        _ => return Err("Bookmarked uri is not a Post resource".into()),
    };

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
        .await
        .map_err(|e| EventProcessorError::IndexWriteFailed {
            message: e.to_string(),
        })?;

    if !existed {
        UserCounts::increment(&user_id, "bookmarks", None)
            .await
            .map_err(|e| EventProcessorError::IndexWriteFailed {
                message: e.to_string(),
            })?;
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

    Bookmark::del_from_index(&user_id, &post_id, &author_id)
        .await
        .map_err(|e| EventProcessorError::IndexWriteFailed {
            message: e.to_string(),
        })?;
    // Update user counts
    UserCounts::decrement(&user_id, "bookmarks", None)
        .await
        .map_err(|e| EventProcessorError::IndexWriteFailed {
            message: e.to_string(),
        })?;

    Ok(())
}
