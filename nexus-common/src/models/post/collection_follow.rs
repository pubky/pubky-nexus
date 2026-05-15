//! A user's follow-relationship to a Collection post.
//!
//! Materialized as a `(:User)-[:FOLLOWS_COLLECTION]->(:Post {kind:'collection'})`
//! edge in Neo4j. The spec primitive `PubkyAppCollectionPointer` lives at
//! `/pub/pubky.app/collections/<owner_id>/<post_id>`; only the follow-pointer
//! flavor (path-owner ≠ homeserver-user) reaches this module — own-pointers
//! are homeserver-only affordances handled by the watcher upstream.
//!
//! The graph-side hard gate is implemented in `queries::put::create_collection_follow`:
//! the MERGE matches `(owner)-[:AUTHORED]->(target:Post {kind:'collection'})` so
//! the edge can never accidentally point at a non-Collection post or claim a
//! mismatched owner. A missing follower / owner / target post yields zero
//! rows → `OperationOutcome::MissingDependency`, which the watcher converts
//! into a retryable error.

use crate::db::{execute_graph_operation, queries, GraphResult, OperationOutcome};

/// Marker type for the (user → collection-post) follow-relationship.
///
/// Stateless on purpose: this primitive has no body fields beyond the
/// `indexed_at` property on the graph edge (mirrors the on-spec
/// `PubkyAppCollectionPointer { created_at }` precedent). All operations
/// are exposed as associated functions.
pub struct CollectionFollow;

impl CollectionFollow {
    /// MERGE a `:FOLLOWS_COLLECTION` edge into the graph.
    ///
    /// Returns:
    /// - `OperationOutcome::CreatedOrDeleted` — the edge was newly created.
    /// - `OperationOutcome::Updated` — the edge already existed; idempotent re-PUT.
    /// - `OperationOutcome::MissingDependency` — follower, owner, or target
    ///   post is not in the graph (or the target isn't `kind=collection`,
    ///   or its `:AUTHORED` edge doesn't connect to `target_owner_id`).
    pub async fn put_to_graph(
        follower_id: &str,
        target_owner_id: &str,
        target_post_id: &str,
        indexed_at: i64,
    ) -> GraphResult<OperationOutcome> {
        let query = queries::put::create_collection_follow(
            follower_id,
            target_owner_id,
            target_post_id,
            indexed_at,
        );
        execute_graph_operation(query).await
    }

    /// DELETE the `:FOLLOWS_COLLECTION` edge if it exists. Idempotent: a
    /// missing edge yields `OperationOutcome::Updated` (no-op), and a
    /// missing follower yields `OperationOutcome::MissingDependency`
    /// (caller decides whether to treat that as a fatal error).
    pub async fn del_from_graph(
        follower_id: &str,
        target_post_id: &str,
    ) -> GraphResult<OperationOutcome> {
        let query = queries::del::delete_collection_follow(follower_id, target_post_id);
        execute_graph_operation(query).await
    }
}
