use crate::db::graph::error::GraphError;
use crate::db::kv::SortOrder;
use crate::db::queries;
use crate::models::error::{ModelError, ModelResult};
use crate::models::resource::tag::TagResource;
use crate::models::resource::ResourceDetails;
use crate::models::tag::traits::TagCollection;
use crate::types::Pagination;
use futures::stream::{self, StreamExt};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};
use utoipa::ToSchema;

use super::view::ResourceView;

#[derive(ToSchema, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum ResourceStreamSource {
    #[default]
    All,
    App {
        app: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResourceSorting {
    #[default]
    Timeline,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Default, Clone)]
pub struct ResourceKeyStream {
    pub resource_ids: Vec<String>,
    /// Latest tag timestamp of the last entry, usable as the `start` cursor for
    /// the next page. `None` when the page is empty.
    ///
    /// The cursor carries only the score, not the id, and `start` is
    /// inclusive, so a resume re-selects every row sharing the cursor score and
    /// takes a `skip` covering the served ones. Timestamps rarely tie, so
    /// `skip=1` resumes a timeline walk.
    pub last_score: Option<u64>,
}

impl ResourceKeyStream {
    pub fn new(resource_ids: Vec<String>, last_score: Option<u64>) -> Self {
        Self {
            resource_ids,
            last_score,
        }
    }

    pub fn from_scored_entries(entries: Vec<(String, i64)>) -> Self {
        let last_score = entries.last().map(|(_, score)| (*score).max(0) as u64);
        let resource_ids = entries.into_iter().map(|(key, _)| key).collect();
        Self::new(resource_ids, last_score)
    }

    pub fn is_empty(&self) -> bool {
        self.resource_ids.is_empty()
    }
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Default)]
pub struct ResourceStream(pub Vec<ResourceView>);

impl ResourceStream {
    /// Get a page of resource IDs for the given source, filters and sorting.
    ///
    /// # Errors
    /// Returns [`ModelError::GraphOperationFailed`] on graph failures, including
    /// `GraphError::QueryTimeout` when the query exceeds its budget.
    pub async fn get_resource_keys(
        source: &ResourceStreamSource,
        pagination: Pagination,
        order: SortOrder,
        sorting: &ResourceSorting,
        tags: Option<&[String]>,
    ) -> ModelResult<ResourceKeyStream> {
        let app = match source {
            ResourceStreamSource::App { app } => Some(app.as_str()),
            ResourceStreamSource::All => None,
        };

        let query = queries::get::resource_stream(app, tags, sorting, &order, &pagination);

        let graph = crate::db::get_neo4j_graph()?;

        // The 10-second budget covers execution AND row streaming: execute()
        // only submits the query and the heavy work (ORDER BY materializes at
        // the first pull) happens while streaming, so a timeout on execute
        // alone lets a slow query run until the HTTP layer's 408.
        let entries = timeout(Duration::from_secs(10), async {
            let mut result = graph.execute(query).await?;

            let mut entries = Vec::new();
            while let Some(row) = result.try_next().await? {
                let id: String = row.get("resource_id")?;
                let score: i64 = row.get("score")?;
                entries.push((id, score));
            }
            Ok::<_, GraphError>(entries)
        })
        .await
        .map_err(|_| GraphError::QueryTimeout)??;

        Ok(ResourceKeyStream::from_scored_entries(entries))
    }

    // -----------------------------------------------------------------------
    // Full ResourceView loading from IDs
    // -----------------------------------------------------------------------

    /// Loads full ResourceView objects from a list of resource IDs.
    // TODO Batch queries:`get_resource_by_id`, `TagResource::get_by_id`
    pub async fn from_listed_resource_ids(
        viewer_id: Option<&str>,
        resource_ids: &[String],
    ) -> ModelResult<Option<Self>> {
        if resource_ids.is_empty() {
            return Ok(None);
        }

        // Bounded to protect the pool; `buffered` preserves order; inputs owned so
        // the future stays `Send`.
        let viewer_id = viewer_id.map(str::to_string);
        let views: Vec<ResourceView> =
            stream::iter(resource_ids.iter().cloned().map(|resource_id| {
                let viewer_id = viewer_id.clone();
                async move {
                    let Some(details) = ResourceDetails::get_by_id(&resource_id).await? else {
                        return Ok::<_, ModelError>(None);
                    };

                    // Load tags via TagResource
                    let tags = TagResource::get_by_id(
                        &resource_id,
                        None,
                        None,
                        Some(5),
                        Some(3),
                        viewer_id.as_deref(),
                        None,
                    )
                    .await?
                    .unwrap_or_default();

                    let taggers_count = tags.iter().map(|t| t.taggers_count).sum();

                    Ok(Some(ResourceView {
                        details,
                        tags,
                        taggers_count,
                    }))
                }
            }))
            .buffered(8)
            .try_collect::<Vec<Option<_>>>()
            .await?
            .into_iter()
            .flatten()
            .collect();

        if views.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ResourceStream(views)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_sorting_accepts_timeline_only() {
        let sorting: ResourceSorting = serde_json::from_str("\"timeline\"").unwrap();
        assert!(matches!(sorting, ResourceSorting::Timeline));

        assert!(
            serde_json::from_str::<ResourceSorting>("\"taggers_count\"").is_err(),
            "the stream no longer sorts by taggers count"
        );
    }
}
