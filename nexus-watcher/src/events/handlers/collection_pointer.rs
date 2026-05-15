//! Handler for `PubkyAppCollectionPointer` events.
//!
//! The spec primitive (path `/pub/pubky.app/collections/<owner_id>/<post_id>`,
//! body `{ created_at: i64 }`) is unified — the same primitive serves two
//! roles, distinguished at read time by comparing the URI host to the path
//! owner:
//!
//! - **own-pointer** (`URI host == path owner`) — sovereign homeserver-side
//!   index entry; Nexus does nothing.
//! - **follow-pointer** (`URI host != path owner`) — subscription; Nexus
//!   materializes a `:FOLLOWS_COLLECTION` edge AND emits a
//!   `FollowCollection` notification to the target owner. DEL removes the
//!   edge silently (no unfollow notification, mirroring the non-mutual
//!   user-unfollow precedent).

use crate::events::retry::event::RetryEvent;
use crate::events::EventProcessorError;
use nexus_common::db::OperationOutcome;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::CollectionFollow;
use pubky_app_specs::{ParsedUri, PubkyId, Resource};
use tracing::debug;

#[tracing::instrument(
    name = "collection_pointer.put",
    skip_all,
    fields(follower_id = %follower_id, target_owner_id = %target_owner_id, target_post_id = %target_post_id)
)]
pub async fn sync_put(
    follower_id: PubkyId,
    target_owner_id: PubkyId,
    target_post_id: String,
    indexed_at: i64,
) -> Result<(), EventProcessorError> {
    // ───── own-pointer: sovereign homeserver-only affordance ─────
    // The user is pointing at their own collection (path owner == URI host).
    // Nexus has nothing to index — the homeserver state suffices for the
    // client to enumerate "my collections" via a prefix scan.
    if follower_id == target_owner_id {
        debug!("Skipping own-pointer indexing (follower == target_owner)");
        return Ok(());
    }

    // ───── follow-pointer: MERGE the :FOLLOWS_COLLECTION edge ─────
    match CollectionFollow::put_to_graph(
        &follower_id,
        &target_owner_id,
        &target_post_id,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => {
            // Idempotent re-PUT — edge exists already, skip notification.
            Ok(())
        }
        OperationOutcome::CreatedOrDeleted => {
            // New follow-edge created. Notify the target owner that someone
            // (the follower) has subscribed to their collection.
            Notification::new_collection_follow(&follower_id, &target_owner_id, &target_post_id)
                .await?;
            Ok(())
        }
        OperationOutcome::MissingDependency => {
            // The target Collection post isn't in the graph yet (or isn't
            // kind=collection, or its :AUTHORED edge doesn't connect to
            // target_owner_id). Try to ingest the target's homeserver in case
            // the post is on its way, then return retryable so the watcher
            // re-attempts once the target post lands.
            if let Err(e) = Homeserver::maybe_ingest_for_user(&target_owner_id).await {
                tracing::error!("Failed to ingest homeserver: {e}");
            }
            let dep_uri = ParsedUri {
                user_id: target_owner_id.clone(),
                resource: Resource::Post(target_post_id.clone()),
            };
            let key = RetryEvent::generate_index_key_from_uri(&dep_uri);
            Err(EventProcessorError::MissingDependency {
                dependency: vec![key],
            })
        }
    }
}

#[tracing::instrument(
    name = "collection_pointer.del",
    skip_all,
    fields(follower_id = %follower_id, target_owner_id = %target_owner_id, target_post_id = %target_post_id)
)]
pub async fn del(
    follower_id: PubkyId,
    target_owner_id: PubkyId,
    target_post_id: String,
) -> Result<(), EventProcessorError> {
    sync_del(follower_id, target_owner_id, target_post_id).await
}

pub async fn sync_del(
    follower_id: PubkyId,
    target_owner_id: PubkyId,
    target_post_id: String,
) -> Result<(), EventProcessorError> {
    // own-pointer DEL: no-op (own-pointers never created a graph edge).
    if follower_id == target_owner_id {
        debug!("Skipping own-pointer del (follower == target_owner)");
        return Ok(());
    }

    // follow-pointer DEL: idempotent. A missing edge yields
    // `OperationOutcome::Updated` from the underlying Cypher (the
    // `existing IS NULL` flag); we don't care which path it took.
    //
    // No notification on unfollow (mirrors the non-mutual user-unfollow case
    // in notification/mod.rs).
    CollectionFollow::del_from_graph(&follower_id, &target_post_id).await?;
    Ok(())
}
