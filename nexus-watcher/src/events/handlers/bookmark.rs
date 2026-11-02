use chrono::Utc;
use nexus_common::db::OperationOutcome;
use nexus_common::models::post::Bookmark;
use nexus_common::models::user::UserIngestor;
use nexus_common::ScopeConfig;
use pubky_app_specs::{ParsedUri, PubkyAppBookmark, PubkyId, Resource};
use tracing::debug;

use super::scope::{finish_out_of_scope_post, index_out_of_scope_post, with_fetch_budget};
use super::utils::fail_on_blacklisted_hs;
use crate::events::EventProcessorError;
use crate::homeserver::HomeserverClient;

/// Indexes a bookmark, fetching an out-of-scope post it points at within one fetch budget.
pub async fn sync_put(
    user_id: PubkyId,
    bookmark: PubkyAppBookmark,
    id: String,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<(), EventProcessorError> {
    with_fetch_budget(put(user_id, bookmark, id, ingestor, client)).await
}

#[tracing::instrument(name = "bookmark.put", skip_all, fields(user_id = %user_id, bookmark_id = %id))]
async fn put(
    user_id: PubkyId,
    bookmark: PubkyAppBookmark,
    id: String,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<(), EventProcessorError> {
    debug!("Indexing bookmark");
    let (author_id, post_id) = bookmark_target(&bookmark.uri)?;
    let parsed_uri = ParsedUri {
        user_id: author_id.clone(),
        resource: Resource::Post(post_id.clone()),
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

/// Returns the author id and post id of the post a bookmark points at.
///
/// A URI that is not a pubky.app Post URI (another scheme, another app's path, an invalid
/// user id, a resource that is not a post) can never become indexable by retrying, so it is
/// rejected with [`EventProcessorError::UnsupportedTarget`], which is not retried.
fn bookmark_target(uri: &str) -> Result<(PubkyId, String), EventProcessorError> {
    let parsed_uri = ParsedUri::try_from(uri).map_err(|e| {
        EventProcessorError::UnsupportedTarget(format!(
            "The bookmarked URI is not a pubky resource: {e}"
        ))
    })?;
    match parsed_uri.resource {
        Resource::Post(post_id) => Ok((parsed_uri.user_id, post_id)),
        other => Err(EventProcessorError::UnsupportedTarget(format!(
            "The bookmarked resource is not a Post, instead is: {other:?}"
        ))),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::retry::RetryScheduler;

    const USER_ID: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";

    #[test]
    fn bookmark_target_rejects_unsupported_targets_without_retry() {
        let cases = [
            "https://example.com/album/123".to_string(),
            format!("pubky://{USER_ID}/pub/mapky/places/1"),
            "pubky://notakey/pub/pubky.app/posts/0033SSE3B1FQ0".to_string(),
            format!("pubky://{USER_ID}/pub/pubky.app/profile.json"),
            format!("pubky://{USER_ID}/pub/pubky.app/unknown_kind/0033SSE3B1FQ0"),
        ];

        for uri in cases {
            let err = bookmark_target(&uri).expect_err(&format!("{uri} must be rejected"));
            assert!(
                matches!(err, EventProcessorError::UnsupportedTarget(_)),
                "{uri}: unexpected error {err:?}"
            );
            assert!(
                !RetryScheduler::should_enqueue_related_event(&err),
                "{uri}: a deterministic rejection must not be retried"
            );
        }
    }

    #[test]
    fn bookmark_target_returns_the_post() {
        assert_eq!(
            bookmark_target(&format!(
                "pubky://{USER_ID}/pub/pubky.app/posts/0033SSE3B1FQ0"
            ))
            .unwrap(),
            (
                PubkyId::try_from(USER_ID).unwrap(),
                "0033SSE3B1FQ0".to_string()
            )
        );
    }
}
