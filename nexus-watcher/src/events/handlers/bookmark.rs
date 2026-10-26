use chrono::Utc;
use nexus_common::db::OperationOutcome;
use nexus_common::models::post::Bookmark;
use nexus_common::models::user::UserIngestor;
use nexus_common::ScopeConfig;
use pubky_app_specs::{ParsedUri, PubkyAppBookmark, PubkyId, Resource};
use tracing::debug;

use super::scope::{finish_out_of_scope_post, index_out_of_scope_post};
use super::utils::fail_on_blacklisted_hs;
use crate::events::EventProcessorError;
use crate::homeserver::HomeserverClient;

#[tracing::instrument(name = "bookmark.put", skip_all, fields(user_id = %user_id, bookmark_id = %id))]
pub async fn sync_put(
    user_id: PubkyId,
    bookmark: PubkyAppBookmark,
    id: String,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<(), EventProcessorError> {
    debug!("Indexing bookmark");
    // Parse the URI to extract author_id and post_id using the updated parse_post_uri
    let parsed_uri =
        ParsedUri::try_from(bookmark.uri.as_str()).map_err(EventProcessorError::generic)?;
    let author_id = parsed_uri.user_id.clone();
    let post_id = match &parsed_uri.resource {
        Resource::Post(id) => id.clone(),
        _ => {
            return Err(EventProcessorError::generic(
                "Bookmarked uri is not a Post resource",
            ))
        }
    };

    finish_out_of_scope_post(&parsed_uri, ingestor, client).await?;

    // Save new bookmark relationship to the graph, only if the bookmarked user exists
    let indexed_at = Utc::now().timestamp_millis();
    if let OperationOutcome::MissingDependency =
        Bookmark::put_to_graph(&author_id, &post_id, &user_id, &id, indexed_at).await?
    {
        // With a scope, the post's author may be outside it: known as an outsider, so the
        // post can be fetched.
        if !matches!(ingestor.scope().config(), ScopeConfig::Everything) {
            fail_on_blacklisted_hs(ingestor.maybe_ingest_author_of_post(&parsed_uri).await)?;
        }
        if index_out_of_scope_post(&parsed_uri, ingestor, client).await? {
            // At most once: the post is indexed now, so it is never fetched again.
            return Box::pin(sync_put(user_id, bookmark, id, ingestor, client)).await;
        }
        let dependency = vec![format!("{author_id}:posts:{post_id}")];
        return Err(EventProcessorError::MissingDependency { dependency });
    }

    // SAVE TO INDEX
    let bookmark_details = Bookmark { id, indexed_at };

    bookmark_details
        .put_to_index(&author_id, &post_id, &user_id)
        .await?;
    Ok(())
}

#[tracing::instrument(name = "bookmark.del", skip_all, fields(user_id = %user_id, bookmark_id = %bookmark_id))]
pub async fn del(user_id: PubkyId, bookmark_id: String) -> Result<(), EventProcessorError> {
    debug!("Deleting bookmark");
    sync_del(user_id, bookmark_id).await
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
