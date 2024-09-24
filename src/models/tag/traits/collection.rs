use axum::async_trait;
use neo4rs::Query;

use crate::{
    db::{
        connectors::neo4j::get_neo4j_graph, graph::exec::exec_single_row,
        kv::index::sorted_sets::Sorting,
    },
    models::tag::{post::POST_TAGS_KEY_PARTS, user::USER_TAGS_KEY_PARTS},
    queries, RedisOps, ScoreAction,
};

use crate::models::tag::TagDetails;

use super::DynError;

/// Trait for managing a collection of tags
///
/// This trait provides methods for querying, indexing, and storing tag-related data
/// for a specific model
#[async_trait]
pub trait TagCollection
where
    Self: RedisOps,
{
    async fn get_by_id(
        user_id: &str,
        extra_param: Option<&str>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
    ) -> Result<Option<Vec<TagDetails>>, DynError> {
        match Self::get_from_index(user_id, extra_param, limit_tags, limit_taggers).await? {
            Some(tag_details) => Ok(Some(tag_details)),
            None => {
                let graph_response = Self::get_from_graph(user_id, extra_param).await?;
                if let Some(tag_details) = graph_response {
                    Self::extend_on_index_miss(user_id, extra_param, &tag_details).await?;
                    return Ok(Some(tag_details));
                }
                Ok(None)
            }
        }
    }
    /// Tries to retrieve the tag collection from multiple index in Redis.
    /// # Arguments
    /// * user_id - The key of the user for whom to retrieve tags.
    /// * extra_param - An optional parameter for specifying additional constraints (e.g., an post_id)
    /// * limit_tags - A limit on the number of tags to retrieve.
    /// * limit_taggers - A limit on the number of taggers to retrieve.
    /// # Returns
    /// A Result containing an optional vector of TagDetails, or an error.
    async fn get_from_index(
        user_id: &str,
        extra_param: Option<&str>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
    ) -> Result<Option<Vec<TagDetails>>, DynError> {
        let limit_tags = limit_tags.unwrap_or(5);
        let limit_taggers = limit_taggers.unwrap_or(5);
        let key_parts = Self::create_sorted_set_key_parts(user_id, extra_param);
        // Get related tags
        match Self::try_from_index_sorted_set(
            &key_parts,
            None,
            None,
            None,
            Some(limit_tags),
            Sorting::Descending,
        )
        .await?
        {
            Some(tag_scores) => {
                let mut tags = Vec::with_capacity(limit_tags);
                for (label, _) in tag_scores.iter() {
                    tags.push(Self::create_label_index(user_id, extra_param, label));
                }
                let tags_ref: Vec<&str> = tags.iter().map(|label| label.as_str()).collect();
                let taggers = Self::try_from_multiple_sets(&tags_ref, Some(limit_taggers)).await?;
                let tag_details_list = TagDetails::from_index(tag_scores, taggers);
                Ok(Some(tag_details_list))
            }
            None => Ok(None),
        }
    }

    /// Retrieves the tag collection from the graph database if it is not found in the index.
    /// # Arguments
    /// * user_id - The key of the user for whom to retrieve tags.
    /// * extra_param - An optional parameter for specifying additional constraints (e.g., an post_id)
    /// # Returns
    /// A Result containing an optional vector of TagDetails, or an error.
    // NAME: get_by_id
    async fn get_from_graph(
        user_id: &str,
        extra_param: Option<&str>,
    ) -> Result<Option<Vec<TagDetails>>, DynError> {
        let mut result;
        {
            // We cannot use LIMIT clause because we need all data related
            let query = Self::graph_query(user_id, extra_param);
            let graph = get_neo4j_graph()?;

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("exists").unwrap_or(false);
            if user_exists {
                let tagged_from: Vec<TagDetails> = row.get("tags").unwrap_or_default();
                return Ok(Some(tagged_from));
            }
        }
        Ok(None)
    }

    async fn extend_on_index_miss(
        user_id: &str,
        extra_param: Option<&str>,
        tags: &[TagDetails],
    ) -> Result<(), DynError> {
        Self::put_to_index(user_id, extra_param, tags).await
    }

    /// Adds the retrieved tags to a sorted set and a set in Redis.
    /// # Arguments
    /// * user_id - The key of the user.
    /// * extra_param - An optional parameter for specifying additional context (e.g., an extra key).
    /// * tags - A slice of TagDetails representing the tags to add.
    /// # Returns
    /// A result indicating success or failure.
    async fn put_to_index(
        user_id: &str,
        extra_param: Option<&str>,
        tags: &[TagDetails],
    ) -> Result<(), DynError> {
        let (tag_scores, (labels, taggers)) = TagDetails::process_tag_details(tags);

        let key_parts = Self::create_sorted_set_key_parts(user_id, extra_param);
        Self::put_index_sorted_set(&key_parts, tag_scores.as_slice()).await?;
        let common_key = Self::create_set_common_key(user_id, extra_param);
        Self::put_multiple_set_indexes(&common_key, &labels, &taggers).await
    }

    // Name?: put_to_score (its obvious that it is sorted set), update_index_score, ???
    async fn put_to_index_score(
        tagged_user_id: &str,
        extra_param: Option<&str>,
        label: &str,
        score_action: ScoreAction,
    ) -> Result<(), DynError> {
        let key: Vec<&str> = match extra_param {
            Some(post_id) => [&POST_TAGS_KEY_PARTS[..], &[tagged_user_id, post_id]].concat(),
            None => [&USER_TAGS_KEY_PARTS[..], &[&tagged_user_id]].concat(),
        };
        Self::put_score_index_sorted_set(&key, &[label], score_action).await
    }

    async fn add_tagger_to_index(
        tagged_user_id: &str,
        extra_param: Option<&str>,
        tagger_user_id: &str,
        tag_label: &str,
    ) -> Result<(), DynError> {
        let key = match extra_param {
            Some(post_id) => vec![tagged_user_id, post_id, tag_label],
            None => vec![tagged_user_id, tag_label],
        };
        Self::put_index_set(&key, &[tagger_user_id]).await
    }

    async fn put_to_graph(
        tagger_user_id: &str,
        tagged_user_id: &str,
        extra_param: Option<&str>,
        tag_id: &str,
        label: &str,
        indexed_at: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let query = match extra_param {
            Some(post_id) => queries::write::create_post_tag(
                tagger_user_id,
                tagged_user_id,
                post_id,
                tag_id,
                label,
                indexed_at,
            ),
            None => queries::write::create_user_tag(
                tagger_user_id,
                tagged_user_id,
                tag_id,
                label,
                indexed_at,
            ),
        };
        exec_single_row(query).await
    }

    /// Returns the unique key parts used to identify a tag in the Redis database
    fn get_tag_prefix<'a>() -> [&'a str; 2];

    /// Creates a Neo4j query to retrieve tags
    /// # Arguments
    /// * user_id - The key of the user for whom to start the retrieval of the tag.
    /// * extra_param - An optional parameter for specifying additional constraints on the query. Options: post_id
    /// # Returns
    /// A query object representing the query to execute in Neo4j.
    fn graph_query(user_id: &str, extra_param: Option<&str>) -> Query {
        match extra_param {
            Some(extra_id) => queries::read::post_tags(user_id, extra_id),
            None => queries::read::user_tags(user_id),
        }
    }

    /// Constructs the index for a sorted set in Redis based on the user key and an optional extra parameter.
    /// # Arguments
    /// * user_id - The key of the user.
    /// * extra_param - An optional parameter for specifying additional context (e.g., an post_id)
    /// # Returns
    /// A vector of strings representing the parts of the key.
    fn create_sorted_set_key_parts<'a>(
        user_id: &'a str,
        extra_param: Option<&'a str>,
    ) -> Vec<&'a str> {
        // Sorted set identifier
        let prefix = Self::get_tag_prefix();
        match extra_param {
            Some(extra_id) => [&prefix[..], &[user_id, extra_id]].concat(),
            None => [&prefix[..], &[user_id]].concat(),
        }
    }

    /// Constructs a slice of common key
    /// # Arguments
    /// * user_id - The key of the user.
    /// * extra_param - An optional parameter for specifying additional context (e.g., an post_id)
    /// # Returns
    /// A vector of string slices representing the parameters.
    fn create_set_common_key<'a>(user_id: &'a str, extra_param: Option<&'a str>) -> Vec<&'a str> {
        match extra_param {
            Some(extra_id) => vec![user_id, extra_id],
            None => vec![user_id],
        }
    }

    /// Constructs an index key based on user key, an optional extra parameter and a tag label.
    /// # Arguments
    /// * user_id - The key of the user.
    /// * extra_param - An optional parameter for specifying additional context (e.g., an post_id)
    /// * label - The label of the tag.
    /// # Returns
    /// A string representing the index key.
    fn create_label_index(user_id: &str, extra_param: Option<&str>, label: &String) -> String {
        match extra_param {
            Some(extra_id) => format!("{}:{}:{}", user_id, extra_id, label),
            None => format!("{}:{}", user_id, label),
        }
    }
}
