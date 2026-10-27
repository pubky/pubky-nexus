use chrono::Utc;
use nexus_common::db::OperationOutcome;
use nexus_common::models::post::Bookmark;
use pubky_social_specs::PubkyId;
use tracing::debug;

use super::utils::bounded;
use crate::events::inputs::{BookmarkInput, ReferenceTarget, ResourceKey};
use crate::events::EventProcessorError;

#[tracing::instrument(name = "bookmark.put", skip_all, fields(user_id = %key.owner_id, bookmark_id = %key.resource_id))]
pub async fn sync_put(
    bookmark: BookmarkInput,
    key: ResourceKey,
) -> Result<(), EventProcessorError> {
    debug!("Indexing bookmark");
    let ResourceKey {
        owner_id: user_id,
        resource_id: bookmark_id,
    } = key;
    let ReferenceTarget::Post(ResourceKey {
        owner_id: author_id,
        resource_id: post_id,
    }) = bookmark.bookmarked.target
    else {
        return Err(EventProcessorError::UnsupportedTarget(format!(
            "The bookmarked resource {} is not a Post, instead is: {}",
            bounded(&bookmark.bookmarked.uri),
            bounded(&format!("{:?}", bookmark.bookmarked.target))
        )));
    };

    // Save new bookmark relationship to the graph, only if the bookmarked user exists
    let indexed_at = Utc::now().timestamp_millis();
    if let OperationOutcome::MissingDependency =
        Bookmark::put_to_graph(&author_id, &post_id, &user_id, &bookmark_id, indexed_at).await?
    {
        return Err(EventProcessorError::MissingDependency {
            dependency: vec![format!("{author_id}:posts:{post_id}")],
        });
    }

    // SAVE TO INDEX
    let bookmark_details = Bookmark {
        id: bookmark_id,
        indexed_at,
    };

    bookmark_details
        .put_to_index(&author_id, &post_id, &user_id)
        .await?;
    Ok(())
}

#[tracing::instrument(name = "bookmark.del", skip_all, fields(user_id = %key.owner_id, bookmark_id = %key.resource_id))]
pub async fn del(key: ResourceKey) -> Result<(), EventProcessorError> {
    debug!("Deleting bookmark");
    sync_del(key.owner_id, key.resource_id).await
}

pub async fn sync_del(user_id: PubkyId, bookmark_id: String) -> Result<(), EventProcessorError> {
    // 1. Read target from graph WITHOUT deleting the edge
    let Some((post_id, author_id)) =
        Bookmark::get_target_from_graph(&user_id, &bookmark_id).await?
    else {
        return Ok(());
    };

    // 2. Redis cleanup (idempotent)
    Bookmark::del_from_index(&user_id, &post_id, &author_id).await?;

    // 3. Graph deletion LAST — ensures data survives for retry if Redis ops fail
    Bookmark::del_from_graph(&user_id, &bookmark_id).await?;

    Ok(())
}
