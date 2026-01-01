use crate::db::queries::get::get_tags;
use crate::db::{fetch_key_from_graph, RedisOps};
use crate::models::create_zero_score_tuples;
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
        let tag_labels_opt = fetch_key_from_graph(get_tags(), "tag_labels").await?;
        let tag_labels: Vec<String> = tag_labels_opt.unwrap_or_default();
        Self::put_to_index(&tag_labels).await
    }

    pub async fn get_by_label(
        label_prefix: &str,
        pagination: &Pagination,
    ) -> Result<Option<Vec<TagSearch>>, DynError> {
        let label_prefix_lowercase = label_prefix.to_lowercase();
        let min_inclusive = format!("[{label_prefix_lowercase}");

        // We mark the end of the label prefix upper bound with the maximum possible Unicode code point
        // Any valid Unicode string will be lexicographically smaller than a string ending with this character
        let max_unicode_char = char::MAX;
        let max_exclusive = format!("({label_prefix_lowercase}{max_unicode_char}");

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

    pub async fn put_to_index(tag_labels: &[String]) -> Result<(), DynError> {
        let elements: Vec<(f64, &str)> = create_zero_score_tuples(tag_labels);
        Self::put_index_sorted_set(&TAGS_LABEL, &elements, None, None).await
    }

    pub async fn del_from_index(tag_label: &str) -> Result<(), DynError> {
        Self::remove_from_index_sorted_set(None, &TAGS_LABEL, &[tag_label]).await
    }
}
