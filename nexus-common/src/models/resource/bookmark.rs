//! Bookmarks of Resources: a public bookmark whose target is not a Pubky post (a URL, a
//! file, another app's object) is a `BOOKMARKED` edge from the user to the target's Resource
//! node, keyed by the same normalized-URI id as Resource tags and post references.

use crate::db::{execute_graph_operation, fetch_key_from_graph, queries};
use crate::db::{GraphResult, OperationOutcome};

pub struct ResourceBookmark;

impl ResourceBookmark {
    /// Writes the user's bookmark of the Resource `resource_id`, creating the Resource on
    /// first reference. `MissingDependency` when the user is not indexed yet.
    pub async fn put_to_graph(
        user_id: &str,
        resource_id: &str,
        uri: &str,
        scheme: &str,
        bookmark_id: &str,
        indexed_at: i64,
    ) -> GraphResult<OperationOutcome> {
        execute_graph_operation(queries::put::create_resource_bookmark(
            user_id,
            resource_id,
            uri,
            scheme,
            bookmark_id,
            indexed_at,
        ))
        .await
    }

    /// Removes the user's Resource bookmark `bookmark_id` and the Resource when nothing else
    /// references it. Returns the Resource id, `None` when there was no such bookmark, so a
    /// repeated delete is a no-op.
    pub async fn del_from_graph(user_id: &str, bookmark_id: &str) -> GraphResult<Option<String>> {
        fetch_key_from_graph(
            queries::del::delete_resource_bookmark(user_id, bookmark_id),
            "resource_id",
        )
        .await
    }
}
