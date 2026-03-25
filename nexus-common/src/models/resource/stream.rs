use crate::db::graph::error::GraphError;
use crate::db::kv::{RedisResult, ScoreAction, SortOrder};
use crate::db::{fetch_row_from_graph, queries, RedisOps};
use crate::models::error::ModelResult;
use crate::models::resource::tag::TagResource;
use crate::models::resource::ResourceDetails;
use crate::models::tag::traits::TagCollection;
use crate::types::Pagination;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::view::ResourceView;

// Redis sorted set key parts
const GLOBAL_TIMELINE: [&str; 3] = ["Resources", "Global", "Timeline"];
const GLOBAL_TAGGERS_COUNT: [&str; 3] = ["Resources", "Global", "TaggersCount"];

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
    TaggersCount,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Default, Clone)]
pub struct ResourceKeyStream {
    pub resource_ids: Vec<String>,
    pub last_score: Option<u64>,
}

impl ResourceKeyStream {
    pub fn new(resource_ids: Vec<String>, last_score: Option<u64>) -> Self {
        Self {
            resource_ids,
            last_score,
        }
    }

    pub fn from_scored_entries(entries: Vec<(String, f64)>) -> Self {
        let last_score = entries.last().map(|(_, score)| score.round() as u64);
        let resource_ids = entries.into_iter().map(|(key, _)| key).collect();
        Self::new(resource_ids, last_score)
    }

    pub fn is_empty(&self) -> bool {
        self.resource_ids.is_empty()
    }
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Default)]
pub struct ResourceStream(pub Vec<ResourceView>);

impl RedisOps for ResourceStream {}

impl ResourceStream {
    // -----------------------------------------------------------------------
    // Index maintenance (called from put_sync_resource / del_sync_resource)
    // -----------------------------------------------------------------------

    /// Add a resource to the global timeline sorted set.
    pub async fn put_to_global_timeline(resource_id: &str, indexed_at: i64) -> RedisResult<()> {
        Self::put_index_sorted_set_static(
            &GLOBAL_TIMELINE
                .map(String::from)
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            &[(indexed_at as f64, resource_id)],
            None,
            None,
        )
        .await
    }

    /// Update the global taggers count sorted set.
    pub async fn update_global_taggers_count(
        resource_id: &str,
        action: ScoreAction,
    ) -> RedisResult<()> {
        Self::put_score_index_sorted_set_static(
            &GLOBAL_TAGGERS_COUNT
                .map(String::from)
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            &[resource_id],
            action,
        )
        .await
    }

    /// Add a resource to a per-app timeline sorted set.
    pub async fn put_to_app_timeline(
        app: &str,
        resource_id: &str,
        indexed_at: i64,
    ) -> RedisResult<()> {
        let key_parts = vec!["Resources", "App", app, "Timeline"];
        Self::put_index_sorted_set_static(
            &key_parts,
            &[(indexed_at as f64, resource_id)],
            None,
            None,
        )
        .await
    }

    /// Update a per-app taggers count sorted set.
    pub async fn update_app_taggers_count(
        app: &str,
        resource_id: &str,
        action: ScoreAction,
    ) -> RedisResult<()> {
        let key_parts = vec!["Resources", "App", app, "TaggersCount"];
        Self::put_score_index_sorted_set_static(&key_parts, &[resource_id], action).await
    }

    /// Add a resource to a per-tag timeline sorted set.
    pub async fn put_to_tag_timeline(
        label: &str,
        resource_id: &str,
        indexed_at: i64,
    ) -> RedisResult<()> {
        let key_parts = vec!["Resources", "Tag", label, "Timeline"];
        Self::put_index_sorted_set_static(
            &key_parts,
            &[(indexed_at as f64, resource_id)],
            None,
            None,
        )
        .await
    }

    /// Update a per-tag taggers count sorted set.
    pub async fn update_tag_taggers_count(
        label: &str,
        resource_id: &str,
        action: ScoreAction,
    ) -> RedisResult<()> {
        let key_parts = vec!["Resources", "Tag", label, "TaggersCount"];
        Self::put_score_index_sorted_set_static(&key_parts, &[resource_id], action).await
    }

    /// Add a resource to a combined app+tag timeline sorted set.
    pub async fn put_to_app_tag_timeline(
        app: &str,
        label: &str,
        resource_id: &str,
        indexed_at: i64,
    ) -> RedisResult<()> {
        let key_parts = vec!["Resources", "App", app, "Tag", label, "Timeline"];
        Self::put_index_sorted_set_static(
            &key_parts,
            &[(indexed_at as f64, resource_id)],
            None,
            None,
        )
        .await
    }

    /// Update a combined app+tag taggers count sorted set.
    pub async fn update_app_tag_taggers_count(
        app: &str,
        label: &str,
        resource_id: &str,
        action: ScoreAction,
    ) -> RedisResult<()> {
        let key_parts = vec!["Resources", "App", app, "Tag", label, "TaggersCount"];
        Self::put_score_index_sorted_set_static(&key_parts, &[resource_id], action).await
    }

    // -----------------------------------------------------------------------
    // Deletion helpers (called from del_sync_resource)
    // -----------------------------------------------------------------------

    /// Remove a resource from the global timeline sorted set.
    pub async fn del_from_global_timeline(resource_id: &str) -> RedisResult<()> {
        Self::remove_from_index_sorted_set(
            None,
            &["Resources", "Global", "Timeline"],
            &[resource_id],
        )
        .await
    }

    /// Remove a resource from a per-app timeline sorted set.
    pub async fn del_from_app_timeline(app: &str, resource_id: &str) -> RedisResult<()> {
        Self::remove_from_index_sorted_set(
            None,
            &["Resources", "App", app, "Timeline"],
            &[resource_id],
        )
        .await
    }

    /// Remove a resource from a per-tag timeline sorted set.
    pub async fn del_from_tag_timeline(label: &str, resource_id: &str) -> RedisResult<()> {
        Self::remove_from_index_sorted_set(
            None,
            &["Resources", "Tag", label, "Timeline"],
            &[resource_id],
        )
        .await
    }

    /// Remove a resource from a combined app+tag timeline sorted set.
    pub async fn del_from_app_tag_timeline(
        app: &str,
        label: &str,
        resource_id: &str,
    ) -> RedisResult<()> {
        Self::remove_from_index_sorted_set(
            None,
            &["Resources", "App", app, "Tag", label, "Timeline"],
            &[resource_id],
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Query methods
    // -----------------------------------------------------------------------

    /// Get resource IDs from the appropriate sorted set.
    pub async fn get_resource_keys(
        source: &ResourceStreamSource,
        pagination: Pagination,
        order: SortOrder,
        sorting: &ResourceSorting,
        tags: &Option<Vec<String>>,
    ) -> ModelResult<ResourceKeyStream> {
        let app = match source {
            ResourceStreamSource::App { app } => Some(app.as_str()),
            ResourceStreamSource::All => None,
        };

        if can_use_index(sorting, app, tags) {
            let key_parts = build_index_key(sorting, app, tags);
            let key_refs: Vec<&str> = key_parts.iter().map(|s| s.as_str()).collect();

            let entries = Self::try_from_index_sorted_set(
                &key_refs,
                pagination.start,
                pagination.end,
                pagination.skip,
                pagination.limit,
                order.clone(),
                None,
            )
            .await?;

            match entries {
                Some(e) if !e.is_empty() => Ok(ResourceKeyStream::from_scored_entries(e)),
                _ => {
                    // Index missing or empty — fall back to Neo4j
                    Self::get_resource_keys_from_graph(app, sorting, tags, &pagination, order).await
                }
            }
        } else {
            // Multi-tag or complex query — use Neo4j directly
            Self::get_resource_keys_from_graph(app, sorting, tags, &pagination, order).await
        }
    }

    async fn get_resource_keys_from_graph(
        app: Option<&str>,
        sorting: &ResourceSorting,
        tags: &Option<Vec<String>>,
        pagination: &Pagination,
        order: SortOrder,
    ) -> ModelResult<ResourceKeyStream> {
        let sorting_field = match sorting {
            ResourceSorting::Timeline => "r.indexed_at",
            ResourceSorting::TaggersCount => "taggers_count",
        };

        let order_direction = match order {
            SortOrder::Ascending => "ASC",
            SortOrder::Descending => "DESC",
        };

        let label_refs: Option<Vec<&str>> = tags
            .as_ref()
            .map(|t| t.iter().map(|s| s.as_str()).collect());

        let query = queries::get::resource_stream(
            app,
            label_refs.as_deref(),
            sorting_field,
            order_direction,
            pagination.skip.unwrap_or(0),
            pagination.limit.unwrap_or(10),
        );

        let graph = crate::db::get_neo4j_graph()?;
        let mut result = graph.execute(query).await.map_err(GraphError::from)?;

        let mut resource_ids = Vec::new();
        let mut last_score = None;

        while let Some(row) = result.try_next().await.map_err(GraphError::from)? {
            let id: String = row.get("resource_id").unwrap_or_default();
            let score: i64 = match sorting {
                ResourceSorting::Timeline => row.get("indexed_at").unwrap_or(0),
                ResourceSorting::TaggersCount => row.get("taggers_count").unwrap_or(0),
            };
            last_score = Some(score as u64);
            resource_ids.push(id);
        }

        Ok(ResourceKeyStream::new(resource_ids, last_score))
    }

    // -----------------------------------------------------------------------
    // Full ResourceView loading from IDs
    // -----------------------------------------------------------------------

    /// Loads full ResourceView objects from a list of resource IDs.
    pub async fn from_listed_resource_ids(
        viewer_id: Option<&str>,
        resource_ids: &[String],
    ) -> ModelResult<Option<Self>> {
        if resource_ids.is_empty() {
            return Ok(None);
        }

        let mut views = Vec::with_capacity(resource_ids.len());

        for resource_id in resource_ids {
            // Load Resource node details from graph
            let query = queries::get::get_resource_by_id(resource_id);
            let maybe_row = fetch_row_from_graph(query).await?;

            let details = match maybe_row {
                Some(row) => ResourceDetails {
                    id: row.get("id").unwrap_or_default(),
                    uri: row.get("uri").unwrap_or_default(),
                    scheme: row.get("scheme").unwrap_or_default(),
                    indexed_at: row.get("indexed_at").unwrap_or(0),
                },
                None => continue, // Resource was deleted between query and load
            };

            // Load tags via TagResource
            let tags =
                TagResource::get_by_id(resource_id, None, None, Some(5), Some(3), viewer_id, None)
                    .await?
                    .unwrap_or_default();

            let taggers_count = tags.iter().map(|t| t.taggers_count).sum();

            views.push(ResourceView {
                details,
                tags,
                taggers_count,
            });
        }

        if views.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ResourceStream(views)))
        }
    }

    // -----------------------------------------------------------------------
    // Static helpers for sorted set operations (no &self needed)
    // -----------------------------------------------------------------------

    async fn put_index_sorted_set_static(
        key_parts: &[&str],
        members: &[(f64, &str)],
        prefix: Option<&str>,
        ttl: Option<i64>,
    ) -> RedisResult<()> {
        Self::put_index_sorted_set(key_parts, members, prefix, ttl).await
    }

    async fn put_score_index_sorted_set_static(
        key_parts: &[&str],
        members: &[&str],
        action: ScoreAction,
    ) -> RedisResult<()> {
        Self::put_score_index_sorted_set(key_parts, members, action).await
    }
}

/// Determines whether a query can be satisfied by a pre-computed Redis sorted set.
fn can_use_index(
    _sorting: &ResourceSorting,
    app: Option<&str>,
    tags: &Option<Vec<String>>,
) -> bool {
    let tag_count = tags.as_ref().map_or(0, |t| t.len());
    match (app, tag_count) {
        (None, 0) => true,    // Global, no filters
        (Some(_), 0) => true, // App filter only
        (None, 1) => true,    // Single tag only
        (Some(_), 1) => true, // App + single tag
        _ => false,           // Multi-tag → Neo4j fallback
    }
}

/// Builds the Redis sorted set key for the given filter combination.
fn build_index_key(
    sorting: &ResourceSorting,
    app: Option<&str>,
    tags: &Option<Vec<String>>,
) -> Vec<String> {
    let sorting_suffix = match sorting {
        ResourceSorting::Timeline => "Timeline",
        ResourceSorting::TaggersCount => "TaggersCount",
    };

    let tag = tags.as_ref().and_then(|t| t.first());

    match (app, tag) {
        (None, None) => vec!["Resources".into(), "Global".into(), sorting_suffix.into()],
        (Some(a), None) => vec![
            "Resources".into(),
            "App".into(),
            a.into(),
            sorting_suffix.into(),
        ],
        (None, Some(label)) => vec![
            "Resources".into(),
            "Tag".into(),
            label.clone(),
            sorting_suffix.into(),
        ],
        (Some(a), Some(label)) => vec![
            "Resources".into(),
            "App".into(),
            a.into(),
            "Tag".into(),
            label.clone(),
            sorting_suffix.into(),
        ],
    }
}
