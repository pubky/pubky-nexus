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
        let tag_labels_opt = retrieve_from_graph(get_tags(), "tag_labels").await?;
        let tag_labels: Vec<String> = tag_labels_opt.unwrap_or_default();
        let tag_labels_slice = &tag_labels.iter().map(String::as_str).collect::<Vec<&str>>();
        Self::put_to_index(tag_labels_slice).await
    }

    pub async fn get_by_label(
        label: &str,
        pagination: &Pagination,
    ) -> Result<Option<Vec<TagSearch>>, DynError> {
        let label_lowercase = label.to_lowercase();
        let min_inclusive = format!("[{label_lowercase}");
        let max_exclusive = format!("({label_lowercase}~");

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

    pub async fn put_to_index(tag_labels: &[&str]) -> Result<(), DynError> {
        let elements: Vec<(f64, &str)> = tag_labels.iter().map(|&label| (0.0, label)).collect();
        Self::put_index_sorted_set(&TAGS_LABEL, &elements, None, None).await
    }
}
