use chrono::Utc;
use nexus_common::db::OperationOutcome;
use nexus_common::models::post::Bookmark;
use pubky_app_specs::{ParsedUri, PubkyAppBookmark, PubkyId, Resource};
use tracing::debug;

use crate::events::EventProcessorError;

#[tracing::instrument(name = "bookmark.put", skip_all, fields(user_id = %user_id, bookmark_id = %id))]
pub async fn sync_put(
    user_id: PubkyId,
    bookmark: PubkyAppBookmark,
    id: String,
) -> Result<(), EventProcessorError> {
    debug!("Indexing bookmark");
    let (author_id, post_id) = bookmark_target(&bookmark.uri)?;

    // Save new bookmark relationship to the graph, only if the bookmarked user exists
    let indexed_at = Utc::now().timestamp_millis();
    if let OperationOutcome::MissingDependency =
        Bookmark::put_to_graph(&author_id, &post_id, &user_id, &id, indexed_at).await?
    {
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
