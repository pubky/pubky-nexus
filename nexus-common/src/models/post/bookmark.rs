use crate::db::{
    exec_single_row, execute_graph_operation, fetch_key_from_graph, queries, GraphResult,
    OperationOutcome,
};
use crate::models::error::ModelResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct Bookmark {
    pub id: String,
    pub indexed_at: i64,
}

impl Bookmark {
    pub async fn put_to_graph(
        author_id: &str,
        post_id: &str,
        user_id: &str,
        bookmark_id: &str,
        indexed_at: i64,
    ) -> GraphResult<OperationOutcome> {
        let query = queries::put::create_post_bookmark(
            user_id,
            author_id,
            post_id,
            bookmark_id,
            indexed_at,
        );

        execute_graph_operation(query).await
    }

    /// The viewer's bookmark on the post, read from the graph; nothing caches it.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        viewer_id: Option<&str>,
    ) -> ModelResult<Option<Bookmark>> {
        // Return None early if no viewer_id supplied
        let Some(viewer_id) = viewer_id else {
            return Ok(None);
        };
        let query = queries::get::post_bookmark(author_id, post_id, viewer_id);
        Ok(fetch_key_from_graph(query, "b").await?)
    }

    pub async fn del_from_graph(user_id: &str, bookmark_id: &str) -> GraphResult<()> {
        exec_single_row(queries::del::delete_bookmark(user_id, bookmark_id)).await
    }
}
