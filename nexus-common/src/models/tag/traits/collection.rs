use crate::db::kv::{RedisResult, ScoreAction, SortOrder};
use crate::db::{
    execute_graph_operation, fetch_row_from_graph, queries, OperationOutcome, RedisOps,
};
use crate::types::DynError;
use async_trait::async_trait;
use neo4rs::Query;
use tracing::error;

use crate::models::tag::{post::POST_TAGS_KEY_PARTS, user::USER_TAGS_KEY_PARTS};

use crate::models::tag::TagDetails;

const CACHE_SORTED_SET_PREFIX: &str = "Cache:Sorted";
pub const CACHE_SET_PREFIX: &str = "Cache";
// TTL, 3HR
const CACHE_TTL: i64 = 3 * 60 * 60;

/// Trait for managing a collection of tags
///
/// This trait provides methods for querying, indexing, and storing tag-related data
/// for a specific model
#[async_trait]
pub trait TagCollection
where
    Self: RedisOps,
{
    /// Retrieves tag details for a given user ID with optional parameters for filtering and limits.
    ///
    /// # Parameters
    /// - `user_id` - A string slice representing the ID of the user for whom the tags are being retrieved
    /// - `extra_param` - An optional string slice used as an additional filter or context in tag retrieval. If it is Some(), the value is post_id
    /// -  skip_tags - The number of tags to skip before retrieving results
    /// - `limit_tags` - An optional limit on the number of tags to retrieve.
    /// - `limit_taggers` - An optional limit on the number of taggers (users who have tagged) to retrieve.
    /// - `viewer_id` - An optional string slice representing the ID of the viewer or requester.
    ///   If `Some`, the function attempts to filter tags based on the viewer's network (WoT - Web of Trust) and the specified depth.
    /// - `depth` - An optional depth value (1-3) for filtering tags within the viewer's Web of Trust. Values outside the range (1-3) are ignored.
    ///
    /// # Behavior
    ///
    /// - If `viewer_id` is provided and `depth` is within the range 1-3, it will retrieve the WoT tags
    /// - If `viewer_id` is not provided or `depth` is out of range, the function retrieves global tags for the user
    /// - The function ensures results from the graph database are cached in the index for faster future retrievals.
    async fn get_by_id(
        user_id: &str,
        extra_param: Option<&str>,
        skip_tags: Option<usize>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
        viewer_id: Option<&str>,
        depth: Option<u8>,
    ) -> Result<Option<Vec<TagDetails>>, DynError> {
        // Query for the tags that are in its WoT
        // Actually we just apply that search to User node
        if viewer_id.is_some() && matches!(depth, Some(1..=3)) {
            match Self::get_from_index(
                user_id,
                viewer_id,
                viewer_id,
                skip_tags,
                limit_tags,
                limit_taggers,
                true,
            )
            .await?
            {
                Some(tag_details) => return Ok(Some(tag_details)),
                None => {
                    let depth = depth.unwrap_or(1);
                    let graph_response =
                        Self::get_from_graph(user_id, viewer_id, Some(depth)).await?;
                    if let Some(tag_details) = graph_response {
                        Self::put_to_index(user_id, viewer_id, &tag_details, true).await?;
                        return Ok(Some(tag_details));
                    }
                    return Ok(None);
                }
            }
        }
        // Get global tags for that user/post
        match Self::get_from_index(
            user_id,
            extra_param,
            viewer_id,
            skip_tags,
            limit_tags,
            limit_taggers,
            false,
        )
        .await?
        {
            Some(tag_details) => Ok(Some(tag_details)),
            None => {
                let graph_response = Self::get_from_graph(user_id, extra_param, None).await?;
                if let Some(tag_details) = graph_response {
                    Self::put_to_index(user_id, extra_param, &tag_details, false).await?;
                    return Ok(Some(tag_details));
                }
                Ok(None)
            }
        }
    }

    /// Tries to retrieve the tag collection from multiple index in Redis.
    /// # Arguments
    /// * user_id - The key of the user for whom to retrieve tags.
    /// * extra_param - An optional parameter for specifying additional constraints: post_id, viewer_id (for WoT search)
    /// * skip_tags - The number of tags to skip before retrieving results
    /// * limit_tags - A limit on the number of tags to retrieve.
    /// * limit_taggers - A limit on the number of taggers to retrieve.
    /// * is_cache - A boolean indicating whether to retrieve tags from the cache or the primary index.
    ///   - `true`: Searches in the cache (e.g., temporary or recently accessed tags).
    ///   - `false`: Searches in the primary index for more persistent data.
    /// # Returns
    /// A Result containing an optional vector of TagDetails, or an error.
    async fn get_from_index(
        user_id: &str,
        extra_param: Option<&str>,
        viewer_id: Option<&str>,
        skip_tags: Option<usize>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
        is_cache: bool,
    ) -> RedisResult<Option<Vec<TagDetails>>> {
        let limit_tags = limit_tags.unwrap_or(5);
        let skip_tags = skip_tags.unwrap_or(0);
        let limit_taggers = limit_taggers.unwrap_or(5);
        let key_parts = Self::create_sorted_set_key_parts(user_id, extra_param, is_cache);
        // Prepare the extra prefix for cache search
        let cache_prefix = match is_cache {
            true => (
                Some(CACHE_SORTED_SET_PREFIX),
                Some(CACHE_SET_PREFIX.to_string()),
            ),
            false => (None, None),
        };
        // Get related tags
        match Self::try_from_index_sorted_set(
            &key_parts,
            None,
            None,
            Some(skip_tags),
            Some(limit_tags),
            SortOrder::Descending,
            cache_prefix.0,
        )
        .await?
        {
            Some(tag_scores) => {
                let mut tags = Vec::with_capacity(limit_tags);
                // TODO: Temporal fix. Should it delete SORTED SET value if score is 0?
                for (label, score) in tag_scores.iter() {
                    // Just process the tags that has score
                    if score >= &1.0 {
                        tags.push(Self::create_label_index(
                            user_id,
                            extra_param,
                            label,
                            is_cache,
                        ));
                    }
                }
                // The index exist but did not match the requested filters
                if tags.is_empty() {
                    return Ok(Some(Vec::new()));
                }

                let tags_ref: Vec<&str> = tags.iter().map(|label| label.as_str()).collect();
                let taggers = Self::try_from_multiple_sets(
                    &tags_ref,
                    cache_prefix.1,
                    viewer_id,
                    Some(limit_taggers),
                )
                .await?;
                let tag_details_list = TagDetails::from_index(tag_scores, taggers);
                Ok(Some(tag_details_list))
            }
            None => Ok(None),
        }
    }

    /// Retrieves the tag collection from the graph database if it is not found in the index.
    /// # Arguments
    /// * user_id - The key of the user for whom to retrieve tags.
    /// * extra_param - An optional parameter for specifying additional constraints (e.g., post_id, viewer_id (for WoT search) )
    /// * `depth` - An optional depth value (1-3) for filtering tags within the viewer's Web of Trust.
    /// # Returns
    /// A Result containing an optional vector of TagDetails, or an error.
    async fn get_from_graph(
        user_id: &str,
        extra_param: Option<&str>,
        depth: Option<u8>,
    ) -> Result<Option<Vec<TagDetails>>, DynError> {
        // We cannot use LIMIT clause because we need all data related
        let query = match depth {
            Some(distance) => queries::get::get_viewer_trusted_network_tags(
                user_id,
                extra_param.unwrap_or_default(),
                distance,
            ),
            None => Self::read_graph_query(user_id, extra_param),
        };

        let maybe_row = fetch_row_from_graph(query).await?;
        if let Some(row) = maybe_row {
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
    /// * extra_param - An optional parameter for specifying additional context (e.g., post_id, viewer_id (for WoT search))
    /// * tags - A slice of TagDetails representing the tags to add.
    /// * is_cache - A boolean indicating whether to retrieve tags from the cache or the primary index.
    /// # Returns
    /// A result indicating success or failure.
    async fn put_to_index(
        user_id: &str,
        extra_param: Option<&str>,
        tags: &[TagDetails],
        is_cache: bool,
    ) -> RedisResult<()> {
        let (tag_scores, (labels, taggers)) = TagDetails::process_tag_details(tags);

        let index_params = match is_cache {
            true => (
                Some(CACHE_SORTED_SET_PREFIX),
                Some(CACHE_TTL),
                Some(CACHE_SET_PREFIX.to_string()),
            ),
            false => (None, None, None),
        };

        let key_parts = Self::create_sorted_set_key_parts(user_id, extra_param, is_cache);
        Self::put_index_sorted_set(
            &key_parts,
            tag_scores.as_slice(),
            index_params.0,
            index_params.1,
        )
        .await?;

        let common_key = Self::create_set_common_key(user_id, extra_param, is_cache);
        Self::put_multiple_set_indexes(
            &common_key,
            &labels,
            &taggers,
            index_params.2,
            index_params.1,
        )
        .await
    }

    /// Updates the score of a label in the appropriate Redis index (user or post) based on the given score action.
    ///
    /// # Arguments
    ///
    /// * `author_id` - A string slice representing the ID of the author whose index is being updated.
    /// * `extra_param` - An optional parameter for specifying additional context, such as a post ID.
    /// * `label` - A string slice representing the label whose score is to be updated.
    /// * `score_action` - The action to perform on the label's score, encapsulated in the `ScoreAction` type
    ///   (e.g., increment, decrement, or set a specific score).
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
        Self::put_score_index_sorted_set(&key, &[label], score_action)
            .await
            .map_err(Into::into)
    }

    /// Adds a tagger (user) to the appropriate Redis index for a specified tag label.
    /// # Arguments
    ///
    /// *`author_id` - A string slice representing the ID of the author whose index is being updated.
    /// * `extra_param` - An optional parameter for specifying additional context, such as a post ID.
    /// * `tagger_user_id` - A string slice representing the ID of the user (tagger) being added to the index.
    /// * `tag_label` - A string slice representing the label of the tag to which the tagger is being added.
    ///
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
        Self::put_index_set(&key, &[tagger_user_id], None, None)
            .await
            .map_err(Into::into)
    }

    /// Inserts a tag relationship into the graph database.
    ///
    /// # Arguments
    ///
    /// - `tagger_user_id` - A string slice representing the ID of the user (tagger) creating the tag.
    /// - `tagged_user_id` - A string slice representing the ID of the user being tagged.
    /// - `extra_param` - An optional parameter for specifying additional context, such as a post ID.
    ///   If `Some`, the function creates a tag relationship associated with a specific post;
    ///   otherwise, it creates a tag relationship between users.
    /// - `tag_id` - A string slice representing the unique identifier of the tag being created.
    /// - `label` - A string slice representing the label of the tag.
    /// - `indexed_at` - A 64-bit integer representing the timestamp (milliseconds)
    ///   when the tag was indexed.
    async fn put_to_graph(
        tagger_user_id: &str,
        tagged_user_id: &str,
        extra_param: Option<&str>,
        tag_id: &str,
        label: &str,
        indexed_at: i64,
    ) -> Result<OperationOutcome, DynError> {
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
        execute_graph_operation(query).await
    }

    /// Reindexes tags for a given author by retrieving data from the graph database and updating the index.
    ///
    /// # Arguments
    ///
    /// - `author_id` - A string slice representing the ID of the author whose tags need to be reindexed.
    /// - `extra_param` - An optional parameter for additional context, such as a post ID.
    ///   If `Some`, the function retrieves and reindexes tags specific to the post;
    ///   if `None`, it reindexes tags globally for the author.
    async fn reindex(author_id: &str, extra_param: Option<&str>) -> Result<(), DynError> {
        match Self::get_from_graph(author_id, extra_param, None).await? {
            Some(tag_user) => Self::put_to_index(author_id, extra_param, &tag_user, false).await?,
            None => error!(
                "{}:{} Could not found tags in the graph",
                author_id,
                extra_param.unwrap_or_default()
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
        let query = queries::del::delete_tag(user_id, tag_id);
        let maybe_row = fetch_row_from_graph(query).await?;

        let Some(row) = maybe_row else {
            return Ok(None);
        };

        let user_id: Option<String> = row.get("user_id").unwrap_or(None);
        let author_id: Option<String> = row.get("author_id").unwrap_or(None);
        let post_id: Option<String> = row.get("post_id").unwrap_or(None);
        let label: String = row.get("label").expect("Query should return tag label");
        Ok(Some((user_id, post_id, author_id, label)))
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
    /// * extra_param - An optional parameter to complete the sorted_set index (post_id | viewer_id)
    /// * is_cache - A boolean indicating whether to retrieve tags from the cache or the primary index.
    /// # Returns
    /// A vector of strings representing the parts of the key.
    fn create_sorted_set_key_parts<'a>(
        user_id: &'a str,
        extra_param: Option<&'a str>,
        is_cache: bool,
    ) -> Vec<&'a str> {
        // Sorted set identifier
        let prefix = Self::get_tag_prefix();
        match extra_param {
            Some(extra_id) => match is_cache {
                // WOT index, the extra param in that case is viewer_id
                true => [&prefix[..], &[extra_id, user_id]].concat(),
                false => [&prefix[..], &[user_id, extra_id]].concat(),
            },
            None => [&prefix[..], &[user_id]].concat(),
        }
    }

    /// Constructs a slice of common key
    /// # Arguments
    /// * user_id - The key of the user.
    /// * extra_param - An optional parameter for specifying additional context (e.g., an post_id)
    /// * is_cache - A boolean indicating whether to retrieve tags from the cache or the primary index.
    /// # Returns
    /// A vector of string slices representing the parameters.
    fn create_set_common_key<'a>(
        user_id: &'a str,
        extra_param: Option<&'a str>,
        is_cache: bool,
    ) -> Vec<&'a str> {
        match extra_param {
            Some(extra_id) => match is_cache {
                true => vec![extra_id, user_id],
                false => vec![user_id, extra_id],
            },
            None => vec![user_id],
        }
    }

    /// Constructs an index key based on user key, an optional extra parameter and a tag label.
    /// # Arguments
    /// * user_id - The key of the user.
    /// * extra_param - An optional parameter for specifying additional context (e.g., an post_id)
    /// * label - The label of the tag.
    /// * is_cache - A boolean indicating whether to retrieve tags from the cache or the primary index.
    /// # Returns
    /// A string representing the index key.
    fn create_label_index(
        user_id: &str,
        extra_param: Option<&str>,
        label: &str,
        is_cache: bool,
    ) -> String {
        match extra_param {
            Some(extra_id) => match is_cache {
                true => format!("{extra_id}:{user_id}:{label}"),
                false => format!("{user_id}:{extra_id}:{label}"),
            },
            None => format!("{user_id}:{label}"),
        }
    }
}
