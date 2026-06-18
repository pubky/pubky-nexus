use chrono::Utc;
use nexus_common::db::OperationOutcome;
use nexus_common::models::post::Bookmark;
use nexus_common::models::user::UserCounts;
use pubky_app_specs::{ParsedUri, PubkyAppBookmark, PubkyId, Resource};
use tracing::debug;

use super::utils::post_is_collection;
use crate::events::EventProcessorError;

#[tracing::instrument(name = "bookmark.put", skip_all, fields(user_id = %user_id, bookmark_id = %id))]
pub async fn sync_put(
    user_id: PubkyId,
    bookmark: PubkyAppBookmark,
    id: String,
) -> Result<(), EventProcessorError> {
    debug!("Indexing new bookmark: {} -> {}", user_id, id);
    // Parse the URI to extract author_id and post_id using the updated parse_post_uri
    let parsed_uri =
        ParsedUri::try_from(bookmark.uri.as_str()).map_err(EventProcessorError::generic)?;
    let author_id = parsed_uri.user_id;
    let post_id = match parsed_uri.resource {
        Resource::Post(id) => id,
        _ => {
            return Err(EventProcessorError::generic(
                "Bookmarked uri is not a Post resource",
            ))
        }
    };

    // Decide collection-follow BEFORE any write: if this lookup failed after the
    // graph/index write, the retry would see `existed = true` and skip the
    // increment forever, leaving a real bookmark uncounted.
    let target_is_collection = post_is_collection(&author_id, &post_id).await?;

    // Save new bookmark relationship to the graph, only if the bookmarked user exists
    let indexed_at = Utc::now().timestamp_millis();
    let existed =
        match Bookmark::put_to_graph(&author_id, &post_id, &user_id, &id, indexed_at).await? {
            OperationOutcome::CreatedOrDeleted => false,
            OperationOutcome::Updated => true,
            OperationOutcome::MissingDependency => {
                let dependency = vec![format!("{author_id}:posts:{post_id}")];
                return Err(EventProcessorError::MissingDependency { dependency });
            }
        };

    // SAVE TO INDEX
    let bookmark_details = Bookmark { id, indexed_at };

    bookmark_details
        .put_to_index(&author_id, &post_id, &user_id)
        .await?;

    // A collection-follow is stored as a bookmark; don't count it.
    if !existed && !target_is_collection {
        UserCounts::increment(&user_id, "bookmarks", None).await?;
    }
    Ok(())
}

#[tracing::instrument(name = "bookmark.del", skip_all, fields(user_id = %user_id, bookmark_id = %bookmark_id))]
pub async fn del(user_id: PubkyId, bookmark_id: String) -> Result<(), EventProcessorError> {
    debug!("Deleting bookmark: {} -> {}", user_id, bookmark_id);
    sync_del(user_id, bookmark_id).await
}

pub async fn sync_del(user_id: PubkyId, bookmark_id: String) -> Result<(), EventProcessorError> {
    // 1. Read target from graph WITHOUT deleting the edge
    let Some((post_id, author_id)) =
        Bookmark::get_target_from_graph(&user_id, &bookmark_id).await?
    else {
        return Ok(());
    };

    // 2. Guard counter decrement: only decrement if bookmark still exists in Redis index
    let existed_in_index = Bookmark::get_from_index(&author_id, &post_id, &user_id)
        .await?
        .is_some();

    // Decide collection-follow BEFORE the cleanup: if this lookup failed after
    // `del_from_index`, the retry would see `existed_in_index = false` and skip
    // the decrement forever, leaving a real bookmark counted.
    let target_is_collection = post_is_collection(&author_id, &post_id).await?;

    // 3. Redis cleanup (idempotent)
    Bookmark::del_from_index(&user_id, &post_id, &author_id).await?;

    // Mirror the PUT gate: collection-follows never incremented `bookmarks`.
    if existed_in_index && !target_is_collection {
        UserCounts::decrement(&user_id, "bookmarks", None).await?;
    }

    // 4. Graph deletion LAST — ensures data survives for retry if Redis ops fail
    Bookmark::del_from_graph(&user_id, &bookmark_id).await?;

    Ok(())
}
