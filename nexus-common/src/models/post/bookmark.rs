use crate::db::kv::RedisResult;
use crate::db::{
    execute_graph_operation, fetch_all_rows_from_graph, fetch_key_from_graph, queries,
    OperationOutcome, RedisOps,
};
use crate::types::DynError;
use neo4rs::Relation;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::PostStream;

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct Bookmark {
    pub id: String,
    pub indexed_at: i64,
}

impl RedisOps for Bookmark {}

impl Bookmark {
    pub async fn put_to_graph(
        author_id: &str,
        post_id: &str,
        user_id: &str,
        bookmark_id: &str,
        indexed_at: i64,
    ) -> Result<OperationOutcome, DynError> {
        let query = queries::put::create_post_bookmark(
            user_id,
            author_id,
            post_id,
            bookmark_id,
            indexed_at,
        );

        execute_graph_operation(query).await
    }

    /// Retrieves counts by user ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Bookmark>, DynError> {
        // Return None early if no viewer_id supplied
        let viewer_id = match viewer_id {
            Some(viewer_id) => viewer_id,
            None => return Ok(None),
        };
        match Self::get_from_index(author_id, post_id, viewer_id).await? {
            Some(counts) => Ok(Some(counts)),
            None => {
                let graph_response = Self::get_from_graph(author_id, post_id, viewer_id).await?;
                if let Some(bookmark) = graph_response {
                    bookmark.put_to_index(author_id, post_id, viewer_id).await?;
                    return Ok(Some(bookmark));
                }
                Ok(None)
            }
        }
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
        viewer_id: &str,
    ) -> RedisResult<Option<Bookmark>> {
        Self::try_from_index_json(&[author_id, post_id, viewer_id], None).await
    }

    /// Retrieves a bookmark from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
        viewer_id: &str,
    ) -> Result<Option<Bookmark>, DynError> {
        let query = queries::get::post_bookmark(author_id, post_id, viewer_id);
        fetch_key_from_graph(query, "b").await
    }

    pub async fn put_to_index(
        &self,
        author_id: &str,
        post_id: &str,
        viewer_id: &str,
    ) -> RedisResult<()> {
        self.put_index_json(&[author_id, post_id, viewer_id], None, None)
            .await?;
        PostStream::add_to_bookmarks_sorted_set(self, viewer_id, post_id, author_id).await
    }

    /// Retrieves all post_keys a user bookmarked from Neo4j
    /// TODO: using in reindex, Refactor
    pub async fn reindex(user_id: &str) -> Result<(), DynError> {
        let query = queries::get::user_bookmarks(user_id);
        let rows = fetch_all_rows_from_graph(query).await?;

        for row in rows {
            if let Some(relation) = row.get::<Option<Relation>>("b")? {
                let bookmark = Bookmark {
                    id: relation.get("id").unwrap_or_default(),
                    indexed_at: relation.get("indexed_at").unwrap_or_default(),
                };
                let author_id = row.get("author_id")?;
                let post_id = row.get("post_id")?;
                bookmark.put_to_index(author_id, post_id, user_id).await?;
            }
        }
        Ok(())
    }

    pub async fn del_from_graph(
        user_id: &str,
        bookmark_id: &str,
    ) -> Result<Option<(String, String)>, DynError> {
        let query = queries::del::delete_bookmark(user_id, bookmark_id);
        let rows = fetch_all_rows_from_graph(query).await?;

        for row in rows {
            let post_id: Option<String> = row.get("post_id").unwrap_or(None);
            let author_id: Option<String> = row.get("author_id").unwrap_or(None);
            if let (Some(post_id), Some(author_id)) = (post_id, author_id) {
                return Ok(Some((post_id, author_id)));
            }
        }
        Ok(None)
    }

    pub async fn del_from_index(
        bookmarker_id: &str,
        post_id: &str,
        author_id: &str,
    ) -> Result<(), DynError> {
        Self::remove_from_index_multiple_json(&[&[author_id, post_id, bookmarker_id]]).await?;
        PostStream::remove_from_bookmarks_sorted_set(bookmarker_id, post_id, author_id).await?;
        Ok(())
    }
}
