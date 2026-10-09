use crate::db::graph::Query;
use crate::db::kv::{RedisResult, ScoreAction, SortOrder};
use crate::db::{
    execute_graph_operation, fetch_all_rows_from_graph, fetch_row_from_graph, queries, GraphResult,
    OperationOutcome, RedisOps,
};
use crate::models::error::ModelResult;
use crate::types::WotDepth;
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::{error, warn};

use crate::models::tag::{post::POST_TAGS_KEY_PARTS, user::USER_TAGS_KEY_PARTS};

use crate::models::tag::TagDetails;

const CACHE_SORTED_SET_PREFIX: &str = "Cache:Sorted";
pub const CACHE_SET_PREFIX: &str = "Cache";
// TTL, 3HR
const CACHE_TTL: i64 = 3 * 60 * 60;
/// Upper bound on caller-supplied tag/tagger page sizes, so an absurd `limit`
/// can't force an unbounded fan-out (mirrors the stream limit cap).
pub(crate) const MAX_TAG_PAGE: usize = 100;

/// Runs a tag query and parses the `exists`/`tags` row shape shared by the
/// global and Web-of-Trust tag queries.
pub(crate) async fn fetch_tag_details(query: Query) -> GraphResult<Option<Vec<TagDetails>>> {
    let maybe_row = fetch_row_from_graph(query).await?;
    if let Some(row) = maybe_row {
        let exists: bool = row.get("exists").unwrap_or(false);
        if exists {
            // A decode failure on an existing post/user is a real error, not a
            // "not found": surface it instead of masking it as `None`.
            let mut tags = row.get::<Vec<TagDetails>>("tags")?;
            // The queries return only `tag_uri`
            for tag in &mut tags {
                tag.relationship = tag.tag_uri.is_some();
            }
            return Ok(Some(tags));
        }
    }
    Ok(None)
}

/// Logs a cache drift: the index lists the viewer as a tagger of the label, but the
/// graph has no edge with a tag address for it.
fn warn_missing_viewer_tag_uri(
    viewer_id: &str,
    user_id: &str,
    extra_param: Option<&str>,
    label: &str,
) {
    warn!(
        "Index flags viewer {} on {}:{}:{}, but the graph has no tag address",
        viewer_id,
        user_id,
        extra_param.unwrap_or_default(),
        label
    );
}

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
    /// - `depth` - An optional validated `WotDepth` for filtering tags through the viewer's Web of Trust; only used together with `viewer_id`.
    ///
    /// # Behavior
    ///
    /// - If `viewer_id` and `depth` are both provided, it retrieves the WoT tags
    /// - Otherwise the function retrieves global tags for the user
    /// - The function ensures results from the graph database are cached in the index for faster future retrievals.
    async fn get_by_id(
        user_id: &str,
        extra_param: Option<&str>,
        skip_tags: Option<usize>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
        viewer_id: Option<&str>,
        depth: Option<WotDepth>,
    ) -> ModelResult<Option<Vec<TagDetails>>> {
        // Query for the tags that are in its WoT
        // Actually we just apply that search to User node
        if let (Some(wot_viewer_id), Some(depth)) = (viewer_id, depth) {
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
                // The WoT cache leaves the viewer out of the taggers, so ask the global
                // taggers sets instead. A missing set can't tell.
                Some(tag_details) => {
                    let keys: Vec<String> = tag_details
                        .iter()
                        .map(|tag| Self::create_label_index(user_id, None, &tag.label, false))
                        .collect();
                    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
                    let membership = Self::check_set_member_multiple(&keys, wot_viewer_id).await?;
                    let tags = tag_details
                        .into_iter()
                        .zip(membership)
                        .map(|(tag, (exists, is_member))| (tag, exists.then_some(is_member)))
                        .collect();
                    return Ok(Some(
                        Self::with_viewer_tag_uris(user_id, None, viewer_id, tags).await?,
                    ));
                }
                None => {
                    let graph_response =
                        Self::get_from_graph(user_id, None, viewer_id, Some(depth)).await?;
                    if let Some(tag_details) = graph_response {
                        // Don't cache an empty WoT result: avoids an empty index
                        // write and a stale-empty window if a trusted tagger tags
                        // this user later.
                        if !tag_details.is_empty() {
                            Self::put_to_index(user_id, viewer_id, &tag_details, true).await?;
                        }
                        return Ok(Some(tag_details));
                    }
                    return Ok(None);
                }
            }
        }
        // Get global tags for that user/post
        match Self::get_from_index_with_viewer_flags(
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
            Some(tags) => {
                let tags = tags
                    .into_iter()
                    .map(|(tag, is_viewer_tagger)| (tag, Some(is_viewer_tagger)))
                    .collect();
                Ok(Some(
                    Self::with_viewer_tag_uris(user_id, extra_param, viewer_id, tags).await?,
                ))
            }
            None => {
                let graph_response =
                    Self::get_from_graph(user_id, extra_param, viewer_id, None).await?;
                if let Some(tag_details) = graph_response {
                    Self::put_to_index(user_id, extra_param, &tag_details, false).await?;
                    return Ok(Some(tag_details));
                }
                Ok(None)
            }
        }
    }

    /// Tries to retrieve the tag collection from multiple index in Redis.
    /// Same arguments as [`Self::get_from_index_with_viewer_flags`]; the viewer's tag is left unset.
    async fn get_from_index(
        user_id: &str,
        extra_param: Option<&str>,
        viewer_id: Option<&str>,
        skip_tags: Option<usize>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
        is_cache: bool,
    ) -> RedisResult<Option<Vec<TagDetails>>> {
        let tags = Self::get_from_index_with_viewer_flags(
            user_id,
            extra_param,
            viewer_id,
            skip_tags,
            limit_tags,
            limit_taggers,
            is_cache,
        )
        .await?;
        Ok(tags.map(|tags| tags.into_iter().map(|(tag, _)| tag).collect()))
    }

    /// Tries to retrieve the tag collection from multiple index in Redis,
    /// flagging the tags `viewer_id` is a tagger of.
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
    /// A Result containing an optional vector of TagDetails, each paired with whether
    /// `viewer_id` is one of its taggers, or an error.
    async fn get_from_index_with_viewer_flags(
        user_id: &str,
        extra_param: Option<&str>,
        viewer_id: Option<&str>,
        skip_tags: Option<usize>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
        is_cache: bool,
    ) -> RedisResult<Option<Vec<(TagDetails, bool)>>> {
        let limit_tags = limit_tags.unwrap_or(5).min(MAX_TAG_PAGE);
        let skip_tags = skip_tags.unwrap_or(0);
        let limit_taggers = limit_taggers.unwrap_or(5).min(MAX_TAG_PAGE);
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

    /// Sets the viewer's stored tag address on each tag, given what the
    /// index says about the viewer (see [`Self::flagged_viewer_tag_uris`]).
    async fn with_viewer_tag_uris(
        user_id: &str,
        extra_param: Option<&str>,
        viewer_id: Option<&str>,
        tags: Vec<(TagDetails, Option<bool>)>,
    ) -> GraphResult<Vec<TagDetails>> {
        let mut uris = match viewer_id {
            Some(viewer_id) => {
                let labels = tags
                    .iter()
                    .map(|(tag, is_viewer_tagger)| (tag.label.clone(), *is_viewer_tagger))
                    .collect();
                Self::flagged_viewer_tag_uris(user_id, extra_param, viewer_id, labels).await?
            }
            None => HashMap::new(),
        };
        Ok(tags
            .into_iter()
            .map(|(mut tag, _)| {
                tag.tag_uri = uris.remove(&tag.label);
                tag.relationship = tag.tag_uri.is_some();
                tag
            })
            .collect())
    }

    /// The viewer's stored tag address per label, read from the graph only for the labels
    /// the index doesn't rule out. Each label comes with what the index says:
    /// - `Some(true)`: the viewer is a tagger. A missing address is logged as a cache drift.
    /// - `Some(false)`: the viewer isn't a tagger, so no graph read.
    /// - `None`: the index can't tell (its set is missing), so the graph decides.
    ///
    /// One graph read for all the labels; none if every label is ruled out.
    async fn flagged_viewer_tag_uris(
        user_id: &str,
        extra_param: Option<&str>,
        viewer_id: &str,
        labels: Vec<(String, Option<bool>)>,
    ) -> GraphResult<HashMap<String, String>> {
        let to_read = labels
            .iter()
            .filter(|(_, is_viewer_tagger)| *is_viewer_tagger != Some(false))
            .map(|(label, _)| label.clone())
            .collect();
        let uris = Self::viewer_tag_uris(user_id, extra_param, viewer_id, to_read).await?;
        for (label, is_viewer_tagger) in &labels {
            if *is_viewer_tagger == Some(true) && !uris.contains_key(label) {
                warn_missing_viewer_tag_uri(viewer_id, user_id, extra_param, label);
            }
        }
        Ok(uris)
    }

    /// The viewer's stored tag address on the target, per label. Labels the viewer hasn't
    /// tagged, or whose edge has no address, are left out. No graph read for no labels.
    async fn viewer_tag_uris(
        user_id: &str,
        extra_param: Option<&str>,
        viewer_id: &str,
        labels: Vec<String>,
    ) -> GraphResult<HashMap<String, String>> {
        if labels.is_empty() {
            return Ok(HashMap::new());
        }
        let query = Self::viewer_tag_uris_query(user_id, extra_param, viewer_id, labels);
        Ok(fetch_all_rows_from_graph(query)
            .await?
            .into_iter()
            .filter_map(|row| Some((row.get("label").ok()?, row.get("uri").ok()?)))
            .collect())
    }

    /// Retrieves the tag collection from the graph database if it is not found in the index.
    /// # Arguments
    /// * user_id - The key of the user for whom to retrieve tags.
    /// * extra_param - An optional parameter for specifying additional constraints: post_id
    /// * viewer_id - The viewer: whose Web of Trust filters the tags with `depth`, and whose
    ///   tag addresses fill `tag_uri` and `relationship` without it.
    /// * `depth` - An optional validated `WotDepth` for filtering tags within the viewer's Web of Trust.
    /// # Returns
    /// A Result containing an optional vector of TagDetails, or an error.
    async fn get_from_graph(
        user_id: &str,
        extra_param: Option<&str>,
        viewer_id: Option<&str>,
        depth: Option<WotDepth>,
    ) -> GraphResult<Option<Vec<TagDetails>>> {
        // We cannot use LIMIT clause because we need all data related
        let query = match depth {
            Some(depth) => queries::get::get_viewer_trusted_network_tags(
                user_id,
                viewer_id.unwrap_or_default(),
                depth,
            ),
            None => Self::read_graph_query(user_id, extra_param, viewer_id),
        };

        fetch_tag_details(query).await
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
    ) -> RedisResult<()> {
        let key: Vec<&str> = match extra_param {
            Some(post_id) => [&POST_TAGS_KEY_PARTS[..], &[author_id, post_id]].concat(),
            None => [&USER_TAGS_KEY_PARTS[..], &[author_id]].concat(),
        };
        Self::put_score_index_sorted_set(&key, &[label], score_action).await
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
    ) -> RedisResult<()> {
        let key = match extra_param {
            Some(post_id) => vec![author_id, post_id, tag_label],
            None => vec![author_id, tag_label],
        };
        Self::put_index_set(&key, &[tagger_user_id], None, None).await
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
    /// - `tag_uri` - The address of the tag file (its event path), stored as the edge `uri`.
    /// - `label` - A string slice representing the label of the tag.
    /// - `indexed_at` - A 64-bit integer representing the timestamp (milliseconds)
    ///   when the tag was indexed.
    async fn put_to_graph(
        tagger_user_id: &str,
        tagged_user_id: &str,
        extra_param: Option<&str>,
        tag_id: &str,
        tag_uri: &str,
        label: &str,
        indexed_at: i64,
    ) -> GraphResult<OperationOutcome> {
        let query = match extra_param {
            Some(post_id) => queries::put::create_post_tag(
                tagger_user_id,
                tagged_user_id,
                post_id,
                tag_id,
                tag_uri,
                label,
                indexed_at,
            ),
            None => queries::put::create_user_tag(
                tagger_user_id,
                tagged_user_id,
                tag_id,
                tag_uri,
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
    async fn reindex(author_id: &str, extra_param: Option<&str>) -> ModelResult<()> {
        match Self::get_from_graph(author_id, extra_param, None, None).await? {
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
    ) -> GraphResult<Option<(Option<String>, Option<String>, Option<String>, String)>> {
        let query = queries::del::delete_tag(user_id, tag_id, None);
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
    /// * viewer_id - Whose tag address fills each tag's `tag_uri` and `relationship`; `None`
    ///   leaves them unset.
    /// # Returns
    /// A query object representing the query to execute in Neo4j.
    fn read_graph_query(
        user_id: &str,
        extra_param: Option<&str>,
        viewer_id: Option<&str>,
    ) -> Query {
        match extra_param {
            Some(extra_id) => queries::get::post_tags(user_id, extra_id, viewer_id),
            None => queries::get::user_tags(user_id, viewer_id),
        }
    }

    /// Creates a Neo4j query returning the viewer's tag addresses (`label`, `uri`) on the target
    /// # Arguments
    /// * user_id - The key of the target user, or the author of the target post.
    /// * extra_param - An optional parameter for specifying the target. Options: post_id
    /// * viewer_id - The tagger whose addresses are read.
    /// * labels - The labels to read.
    fn viewer_tag_uris_query(
        user_id: &str,
        extra_param: Option<&str>,
        viewer_id: &str,
        labels: Vec<String>,
    ) -> Query {
        match extra_param {
            Some(post_id) => {
                queries::get::viewer_post_tag_uris(user_id, post_id, viewer_id, labels)
            }
            None => queries::get::viewer_user_tag_uris(user_id, viewer_id, labels),
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
