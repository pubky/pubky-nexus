use crate::db::kv::RedisResult;
use crate::db::queries::get::get_tags;
use crate::db::{fetch_key_from_graph, RedisOps};
use crate::models::create_zero_score_tuples;
use crate::models::error::ModelResult;
use crate::types::Pagination;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const TAGS_LABEL: [&str; 2] = ["Tags", "Label"];

/// What the prefix search does to a prefix before matching it against the stored labels. The
/// index holds every label as its writer folded it: the `pubky.app` rule lowercases every
/// letter, the `social/v1` rule only ASCII, so a label can hold a non-ASCII capital.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixFold {
    /// Lowercase the whole prefix with the Unicode rule, the `pubky.app` fold: a prefix written
    /// with a non-ASCII capital matches the labels stored in lowercase.
    Lowercase,
    /// Match the prefix as given: for a caller that already folded it as the labels it looks
    /// for were folded, so a non-ASCII capital matches a label that kept one.
    AsGiven,
}

/// Represents a single search result of a tag search
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct TagSearch(String);

impl RedisOps for TagSearch {}

impl TagSearch {
    /// Retrieves tags from the Neo4j graph and updates global sorted set
    pub async fn reindex() -> ModelResult<()> {
        let tag_labels_opt = fetch_key_from_graph(get_tags(), "tag_labels").await?;
        let tag_labels: Vec<String> = tag_labels_opt.unwrap_or_default();
        Self::put_to_index(&tag_labels).await.map_err(Into::into)
    }

    /// The labels starting with `label_prefix` lowercased with the Unicode rule:
    /// [`Self::get_by_prefix`] with [`PrefixFold::Lowercase`].
    pub async fn get_by_label(
        label_prefix: &str,
        pagination: &Pagination,
    ) -> RedisResult<Option<Vec<TagSearch>>> {
        Self::get_by_prefix(label_prefix, PrefixFold::Lowercase, pagination).await
    }

    /// The labels starting with `prefix`, folded as `fold` says, in lexicographic order.
    pub async fn get_by_prefix(
        prefix: &str,
        fold: PrefixFold,
        pagination: &Pagination,
    ) -> RedisResult<Option<Vec<TagSearch>>> {
        let prefix = match fold {
            PrefixFold::Lowercase => prefix.to_lowercase(),
            PrefixFold::AsGiven => prefix.to_string(),
        };
        let min_inclusive = format!("[{prefix}");

        // We mark the end of the label prefix upper bound with the maximum possible Unicode code point
        // Any valid Unicode string will be lexicographically smaller than a string ending with this character
        let max_unicode_char = char::MAX;
        let max_exclusive = format!("({prefix}{max_unicode_char}");

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

    pub async fn put_to_index(tag_labels: &[String]) -> RedisResult<()> {
        let elements: Vec<(f64, &str)> = create_zero_score_tuples(tag_labels);
        Self::put_index_sorted_set(&TAGS_LABEL, &elements, None, None).await
    }

    pub async fn del_from_index(tag_label: &str) -> RedisResult<()> {
        Self::remove_from_index_sorted_set(None, &TAGS_LABEL, &[tag_label]).await
    }
}
