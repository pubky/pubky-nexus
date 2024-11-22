use crate::types::DynError;
use axum::async_trait;
use neo4rs::Query;

use crate::{
    db::{
        connectors::neo4j::get_neo4j_graph, graph::exec::exec_boolean_row,
        kv::index::sorted_sets::SortOrder,
    },
    models::tag::{post::POST_TAGS_KEY_PARTS, user::USER_TAGS_KEY_PARTS},
    queries, RedisOps, ScoreAction,
};

use crate::models::tag::TagDetails;

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
                    Self::put_to_index(user_id, extra_param, &tag_details).await?;
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
            SortOrder::Descending,
        )
        .await?
        {
            Some(tag_scores) => {
                let mut tags = Vec::with_capacity(limit_tags);
                // TODO: Temporal fix. Should it delete SORTED SET value if score is 0?
                for (label, score) in tag_scores.iter() {
                    // Just process the tags that has score
                    if score >= &1.0 {
                        tags.push(Self::create_label_index(user_id, extra_param, label));
                    }
                }
                if tags.is_empty() {
                    return Ok(None);
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
            let query = Self::read_graph_query(user_id, extra_param);
            let graph = get_neo4j_graph()?;

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let user_exists: bool = row.get("exists").unwrap_or(false);
            if user_exists {
                match row.get::<Vec<TagDetails>>("tags") {
                    Ok(tagged_from) => return Ok(Some(tagged_from)),
                    Err(_e) => return Ok(None),
                }
            }
        }
        Ok(None)
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

    async fn update_index_score(
        author_id: &str,
        extra_param: Option<&str>,
        label: &str,
        score_action: ScoreAction,
    ) -> Result<(), DynError> {
        let key: Vec<&str> = match extra_param {
            Some(post_id) => [&POST_TAGS_KEY_PARTS[..], &[author_id, post_id]].concat(),
            None => [&USER_TAGS_KEY_PARTS[..], &[author_id]].concat(),
        };
        Self::put_score_index_sorted_set(&key, &[label], score_action).await
    }

    async fn add_tagger_to_index(
        author_id: &str,
        extra_param: Option<&str>,
        tagger_user_id: &str,
        tag_label: &str,
    ) -> Result<(), DynError> {
        let key = match extra_param {
            Some(post_id) => vec![author_id, post_id, tag_label],
            None => vec![author_id, tag_label],
        };
        Self::put_index_set(&key, &[tagger_user_id], None).await
    }

    async fn put_to_graph(
        tagger_user_id: &str,
        tagged_user_id: &str,
        extra_param: Option<&str>,
        tag_id: &str,
        label: &str,
        indexed_at: i64,
    ) -> Result<bool, DynError> {
        let query = match extra_param {
            Some(post_id) => queries::put::create_post_tag(
                tagger_user_id,
                tagged_user_id,
                post_id,
                tag_id,
                label,
                indexed_at,
            ),
            None => queries::put::create_user_tag(
                tagger_user_id,
                tagged_user_id,
                tag_id,
                label,
                indexed_at,
            ),
        };
        exec_boolean_row(query).await
    }

    async fn reindex(author_id: &str, extra_param: Option<&str>) -> Result<(), DynError> {
        match Self::get_from_graph(author_id, extra_param).await? {
            Some(tag_user) => Self::put_to_index(author_id, extra_param, &tag_user).await?,
            None => log::error!(
                "{}:{} Could not found tags in the graph",
                author_id,
                extra_param.unwrap()
            ),
        }
        Ok(())
    }

    /// Deletes a tag relationship between a user and a tagged target (User or Post) in the graph database.
    /// # Arguments
    /// * `user_id` - The ID of the user who owns the tag relationship.
    /// * `tag_id` - The ID of the tag to be deleted.
    ///
    /// # Returns
    ///
    /// A `Result` containing:
    /// * `Some((Option<String>, Option<String>, Option<String>, String))`: If the tag was found and deleted:
    ///   - `Option<String>` for the `user_id` of the target (if the target is a user, otherwise `None`),
    ///   - `Option<String>` for the `post_id` of the target (if the target is a post, otherwise `None`),
    ///   - `Option<String>` for the `author_id` of the post (if applicable, otherwise `None`),
    ///   - `String` for the tag label.
    /// * `None` if no matching tag relationship is found.
    ///
    /// # Errors
    ///
    /// Returns a boxed `std::error::Error` if there is any issue querying or executing the delete operation in Neo4j.
    async fn del_from_graph(
        user_id: &str,
        tag_id: &str,
    ) -> Result<Option<(Option<String>, Option<String>, Option<String>, String)>, DynError> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::put::delete_tag(user_id, tag_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        if let Some(row) = result.next().await? {
            let user_id: Option<String> = row.get("user_id").unwrap_or(None);
            let author_id: Option<String> = row.get("author_id").unwrap_or(None);
            let post_id: Option<String> = row.get("post_id").unwrap_or(None);
            let label: String = row.get("label").expect("Query should return tag label");
            return Ok(Some((user_id, post_id, author_id, label)));
        }
        Ok(None)
    }

    /// Returns the unique key parts used to identify a tag in the Redis database
    fn get_tag_prefix<'a>() -> [&'a str; 2];

    /// Creates a Neo4j query to retrieve tags
    /// # Arguments
    /// * user_id - The key of the user for whom to start the retrieval of the tag.
    /// * extra_param - An optional parameter for specifying additional constraints on the query. Options: post_id
    /// # Returns
    /// A query object representing the query to execute in Neo4j.
    fn read_graph_query(user_id: &str, extra_param: Option<&str>) -> Query {
        match extra_param {
            Some(extra_id) => queries::get::post_tags(user_id, extra_id),
            None => queries::get::user_tags(user_id),
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
