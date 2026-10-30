use chrono::Utc;
use nexus_common::db::OperationOutcome;
use nexus_common::models::post::Bookmark;
use nexus_common::models::resource::bookmark::ResourceBookmark;
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
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
    let (author_id, post_id) = match bookmark.bookmarked.target {
        ReferenceTarget::Post(ResourceKey {
            owner_id,
            resource_id,
        }) => (owner_id, resource_id),
        // A profile is not a Resource: bookmarks of users stay unsupported.
        ReferenceTarget::User(_) => {
            return Err(EventProcessorError::UnsupportedTarget(format!(
                "The bookmarked resource {} is a user profile, not a Post or a Resource",
                bounded(&bookmark.bookmarked.uri)
            )))
        }
        // Any other target (a URL, a file, another app's object) is a Resource, the same
        // routing Resource tags use.
        ReferenceTarget::File(_) | ReferenceTarget::Tag { .. } | ReferenceTarget::Other => {
            return sync_put_resource(&bookmark.bookmarked.uri, user_id, bookmark_id).await
        }
    };

    // Save new bookmark relationship to the graph, only if the bookmarked user exists
    let indexed_at = Utc::now().timestamp_millis();
    if let OperationOutcome::MissingDependency =
        Bookmark::put_to_graph(&author_id, &post_id, &user_id, &bookmark_id, indexed_at).await?
    {
        let dependency = vec![format!("{author_id}:posts:{post_id}")];
        return Err(EventProcessorError::MissingDependency { dependency });
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
        // Not a post bookmark: a Resource bookmark has no index entries, so deleting its edge
        // (and the Resource when it is left unreferenced) is the whole delete. A repeat is a no-op.
        ResourceBookmark::del_from_graph(&user_id, &bookmark_id).await?;
        return Ok(());
    };

    // 2. Redis cleanup (idempotent)
    Bookmark::del_from_index(&user_id, &post_id, &author_id).await?;

    // 3. Graph deletion LAST — ensures data survives for retry if Redis ops fail
    Bookmark::del_from_graph(&user_id, &bookmark_id).await?;

    Ok(())
}

/// Indexes a bookmark of a non-post target as a `BOOKMARKED` edge to its Resource. Resource
/// views are read from the graph, so there is no index to write.
async fn sync_put_resource(
    uri: &str,
    user_id: PubkyId,
    bookmark_id: String,
) -> Result<(), EventProcessorError> {
    let (normalized, scheme) = normalize_uri(uri).map_err(|e| {
        EventProcessorError::UnsupportedTarget(format!(
            "The bookmarked URI {} cannot be a Resource: {}",
            bounded(uri),
            bounded(&e)
        ))
    })?;
    let indexed_at = Utc::now().timestamp_millis();
    match ResourceBookmark::put_to_graph(
        &user_id,
        &resource_id(&normalized),
        &normalized,
        &scheme,
        &bookmark_id,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::MissingDependency => Err(EventProcessorError::MissingDependency {
            dependency: vec![user_id.to_string()],
        }),
        OperationOutcome::CreatedOrDeleted | OperationOutcome::Updated => Ok(()),
    }
}
