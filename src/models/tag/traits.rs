use std::error::Error;

use axum::async_trait;
use neo4rs::Query;

use crate::{
    db::{connectors::neo4j::get_neo4j_graph, kv::index::sorted_sets::Sorting},
    queries, RedisOps,
};

use super::TagDetails;

type DynError = Box<dyn Error + Send + Sync>;

/// Trait for managing a collection of tags
///
/// This trait provides methods for querying, indexing, and storing tag-related data
/// for a specific model
#[async_trait]
pub trait TagCollection
where
    Self: RedisOps,
{
    /// Retrieves the tag collection, either from an index or directly from the graph database.
    /// # Arguments
    /// * user_id - The key of the user for whom to retrieve tags.
    /// * extra_param - An optional parameter for specifying additional constraints (e.g., an post_id)
    /// * limit_tags - An optional limit on the number of tags to retrieve.
    /// * limit_taggers - An optional limit on the number of taggers to retrieve.
    /// # Returns
    /// A Result containing an optional vector of TagDetails, or an error.
    async fn get_by_id(
        user_id: &str,
        extra_param: Option<&str>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
    ) -> Result<Option<Vec<TagDetails>>, DynError> {
        // TODO: Not sure if this is the place to do or in the endpoint
        let limit_tags = limit_tags.unwrap_or(5);
        let limit_taggers = limit_taggers.unwrap_or(5);
        match Self::try_from_index(user_id, extra_param, limit_tags, limit_taggers).await? {
            Some(counts) => Ok(Some(counts)),
            None => Self::get_from_graph(user_id, extra_param).await,
        }
    }

    /// Tries to retrieve the tag collection from an index in Redis.
    /// # Arguments
    /// * user_id - The key of the user for whom to retrieve tags.
    /// * extra_param - An optional parameter for specifying additional constraints (e.g., an post_id)
    /// * limit_tags - A limit on the number of tags to retrieve.
    /// * limit_taggers - A limit on the number of taggers to retrieve.
    /// # Returns
    /// A Result containing an optional vector of TagDetails, or an error.
    async fn try_from_index(
        user_id: &str,
        extra_param: Option<&str>,
        limit_tags: usize,
        limit_taggers: usize,
    ) -> Result<Option<Vec<TagDetails>>, DynError> {
        let key_parts = Self::create_sorted_set_key_parts(user_id, extra_param);
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
                Self::add_to_label_sets(user_id, extra_param, &tagged_from).await?;
                return Ok(Some(tagged_from));
            }
        }
        Ok(None)
    }

    /// Adds the retrieved tags to a sorted set and a set in Redis.
    /// # Arguments
    /// * user_id - The key of the user.
    /// * extra_param - An optional parameter for specifying additional context (e.g., an extra key).
    /// * tags - A slice of TagDetailsrepresenting the tags to add.
    /// # Returns
    /// A result indicating success or failure.
    async fn add_to_label_sets(
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
