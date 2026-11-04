use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Bookmark, PostCounts, PostDetails, PostRelationships};
use crate::models::error::ModelResult;
use crate::models::tag::post::TagPost;
use crate::models::tag::traits::TagCollection;
use crate::models::tag::TagDetails;

/// Represents a Pubky user with relational data including tags, counts, and relationship with a viewer.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct PostView {
    /// The post's rating summary. Not serialized: `/v0` is frozen and serves this view as it
    /// is, so the summary reaches a client only through the `/v1` views, which copy it.
    #[serde(skip)]
    pub rating: crate::models::social_event::RatingSummary,
    pub details: PostDetails,
    pub counts: PostCounts,
    pub tags: Vec<TagDetails>,
    pub relationships: PostRelationships,
    pub bookmark: Option<Bookmark>,
}

impl PostView {
    /// Retrieves a user ID, checking the cache first and then the graph database.
    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        viewer_id: Option<&str>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
    ) -> ModelResult<Option<Self>> {
        // Perform all operations concurrently
        let (details, counts, bookmark, relationships) = tokio::try_join!(
            PostDetails::get_by_id(author_id, post_id),
            PostCounts::get_by_id(author_id, post_id),
            Bookmark::get_by_id(author_id, post_id, viewer_id),
            PostRelationships::get_by_id(author_id, post_id),
        )?;

        let details = match details {
            None => return Ok(None),
            Some(details) => details,
        };

        let counts = counts.unwrap_or_default();
        let relationships = relationships.unwrap_or_default();

        // Before fetching post tags, check if the post has any tags
        // Without this check, the index search will return a NONE because the tag index
        // doesn't exist, leading us to query the graph unnecessarily, assuming the data wasn't indexed
        let tags = match counts.tags {
            0 => Vec::new(),
            _ => {
                TagPost::get_by_id(
                    author_id,
                    Some(post_id),
                    None,
                    limit_tags,
                    limit_taggers,
                    viewer_id,
                    None, // Avoid by default WoT tags in a Post
                )
                .await?
                .unwrap_or_default()
            }
        };

        // Every rating is a reply, so only a post with replies can have one. A failed read
        // serves the view without a rating rather than failing it.
        let rating = match counts.replies {
            0 => Default::default(),
            _ => crate::models::social_event::rating_summary(&details.uri)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!(post = %details.uri, %error, "Rating summary unavailable");
                    Default::default()
                }),
        };
        Ok(Some(Self {
            rating,
            details,
            counts,
            bookmark,
            relationships,
            tags,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{get_neo4j_graph, graph::Query};
    use crate::models::post::PostKind;
    use crate::{StackConfig, StackManager};
    use tracing_test::traced_test;

    /// Every rating is a reply, so a post with no replies has no rating: its view is served
    /// without reading one from the graph.
    #[tokio_shared_rt::test(shared)]
    #[traced_test]
    async fn a_post_without_replies_is_viewed_without_a_rating_read(
    ) -> Result<(), crate::types::DynError> {
        let mut config = StackConfig::default();
        // Every query is logged with its label, so the test sees which ones ran
        config.db.neo4j.slow_query_logging_threshold_ms = Some(0);
        StackManager::setup(&config).await?;
        let author = pubky::Keypair::random().public_key().to_z32();
        let id = format!("{:013X}", chrono::Utc::now().timestamp_micros());
        get_neo4j_graph()?
            .run(
                Query::new("view_rating_user", "CREATE (:User {id:$author})")
                    .param("author", author.clone()),
            )
            .await?;
        let post = PostDetails {
            id: id.clone(),
            author: author.clone(),
            uri: format!("pubky://{author}/pub/social/v1/posts/{id}"),
            content: "no replies".into(),
            indexed_at: 1234,
            kind: PostKind::Short,
            attachments: vec![],
            lock: None,
            deleted: false,
        };
        post.put_to_graph(None, None).await?;

        let view = PostView::get_by_id(&author, &id, None, None, None)
            .await?
            .expect("the post exists");
        assert_eq!(view.counts.replies, 0);
        assert!(logs_contain("Slow Neo4j query"), "every query is logged");
        assert!(
            !logs_contain("query=rating_summary"),
            "a post with no replies has no rating to read"
        );

        get_neo4j_graph()?
            .run(
                Query::new(
                    "view_rating_cleanup",
                    "MATCH (u:User {id:$author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p,u",
                )
                .param("author", author),
            )
            .await?;
        Ok(())
    }
}
