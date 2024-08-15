use super::PostView;
use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::queries;
use neo4rs::BoltMap;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents a thread of posts, starting from the root post and including all replies.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostThread {
    pub root_post: PostView,
    pub replies: Vec<PostView>,
}

impl PostThread {
    /// Retrieves the thread by author ID and post ID with pagination.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        viewer_id: Option<&str>,
        skip: usize,
        limit: usize,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        // Fetch the root post and its replies from Neo4j.
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get_thread(author_id, post_id, skip, limit);
            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let root_post_view = PostView::get_by_id(author_id, post_id, viewer_id).await?;

            let root_post_view = match root_post_view {
                None => return Ok(None),
                Some(root_post_view) => root_post_view,
            };

            // Extract replies and their authors
            let replies: Vec<BoltMap> = row.get("replies")?;

            let mut replies_view = Vec::new();
            for reply in replies {
                let reply_id: String = reply.get("reply_id")?;
                let reply_author_id: String = reply.get("author_id")?;

                let reply_view = PostView::get_by_id(&reply_author_id, &reply_id, viewer_id)
                    .await?
                    .unwrap_or_default();
                replies_view.push(reply_view);
            }

            return Ok(Some(PostThread {
                root_post: root_post_view,
                replies: replies_view,
            }));
        }

        Ok(None)
    }
}
