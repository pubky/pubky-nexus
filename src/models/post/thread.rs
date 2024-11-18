use super::PostView;
use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::queries;
use crate::routes::v0::post::ThreadQuery;
use neo4rs::BoltMap;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents a thread of posts, starting from the root post and including all replies.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct PostThread {
    pub root_post: PostView,
    pub replies: Vec<PostView>,
}

impl PostThread {
    /// Retrieves the thread by author ID and post ID with pagination.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        endpoint_query: ThreadQuery,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        // Set the default params
        let skip = endpoint_query.pagination.skip.unwrap_or(0);
        let limit = endpoint_query.pagination.limit.unwrap_or(6).min(20);
        let depth = endpoint_query.depth.unwrap_or(1).min(3);
        let viewer_id = endpoint_query.viewer_id.as_deref();

        // Fetch the root post view
        let root_post_view = PostView::get_by_id(author_id, post_id, viewer_id, None, None).await?;

        let root_post_view = match root_post_view {
            None => return Ok(None),
            Some(root_post_view) => root_post_view,
        };

        // Fetch the root post and its replies from Neo4j.
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get::get_thread(author_id, post_id, depth, skip, limit);
            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        let replies = match result.next().await? {
            Some(row) => {
                let replies: Vec<BoltMap> = row.get("replies").unwrap_or(Vec::new());

                let mut replies_view = Vec::with_capacity(replies.len());

                for reply in replies {
                    let reply_id: String = reply.get("reply_id").unwrap_or(String::new());
                    let reply_author_id: String = reply.get("author_id").unwrap_or(String::new());

                    // Make sure we have both variables
                    if !reply_id.is_empty() && !reply_author_id.is_empty() {
                        let reply_view =
                            PostView::get_by_id(&reply_author_id, &reply_id, viewer_id, None, None)
                                .await?
                                .unwrap_or_default();
                        replies_view.push(reply_view);
                    }
                }
                replies_view
            }
            None => Vec::new(),
        };

        Ok(Some(PostThread {
            root_post: root_post_view,
            replies,
        }))
    }
}
