use crate::db::queries::get::get_tags;
use crate::db::{retrieve_from_graph, RedisOps};
use crate::types::DynError;
use crate::types::Pagination;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const TAGS_LABEL: [&str; 2] = ["Tags", "Label"];

/// Represents a single search result of a tag search
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct TagSearch(String);

impl RedisOps for TagSearch {}

impl TagSearch {
    /// Retrieves tags from the Neo4j graph and updates global sorted set
    pub async fn reindex() -> Result<(), DynError> {
        let tag_labels: Option<Vec<String>> = retrieve_from_graph(get_tags(), "tag_labels").await?;
        for tag_label in tag_labels.unwrap_or_default() {
            Self::put_index_sorted_set(&TAGS_LABEL, &[(0.0, &tag_label)], None, None).await?;
        }

        Ok(())
    }

    pub async fn get_by_label(
        label: &str,
        pagination: &Pagination,
    ) -> Result<Option<Vec<TagSearch>>, DynError> {
        let label_lowercase = label.to_lowercase();
        let min_inclusive = format!("[{}", label_lowercase);
        let max_exclusive = format!("({}~", label_lowercase);

        Self::try_from_index_sorted_set_lex(
            &TAGS_LABEL,
            &min_inclusive,
            &max_exclusive,
            pagination.skip,
            pagination.limit,
        )
        .await
        .map(|opt| opt.map(|list| list.into_iter().map(TagSearch).collect()))
    }

    pub async fn put_to_index(tag_label: &str) -> Result<(), DynError> {
        let elements = [(0.0, tag_label)];
        Self::put_index_sorted_set(&TAGS_LABEL, &elements, Some("Sorted"), None).await
    }

    // TODO Needed? When are tags removed?
    pub async fn del_from_index(tag_label: &str) -> Result<(), DynError> {
        Self::remove_from_index_sorted_set(Some("Sorted"), &TAGS_LABEL, &[tag_label]).await
    }
}
