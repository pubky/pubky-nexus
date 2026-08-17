use super::{UserDetails, USER_DELETED_SENTINEL};
use crate::db::kv::{RedisResult, SortOrder};
use crate::db::{fetch_all_rows_from_graph, get_neo4j_graph, queries, GraphError, RedisOps};
use crate::models::create_zero_score_tuples;
use crate::models::error::ModelResult;
use crate::models::tag::user::{TagUser, USER_TAGS_KEY_PARTS};
use crate::models::traits::Collection;
use crate::types::Pagination;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};
use utoipa::ToSchema;

pub const USER_NAME_KEY_PARTS: [&str; 2] = ["Users", "Name"];
pub const USER_ID_KEY_PARTS: [&str; 2] = ["Users", "ID"];
pub const TAG_GLOBAL_USER_TAGGERS: [&str; 4] = ["Tags", "Global", "User", "Taggers"];
const EVICT_PAGE_SIZE: usize = 500;

/// Represents a single result of a "users by tags" search: a user whose profile
/// carries at least one of the searched tag labels, and how many distinct
/// taggers applied them (summed across the searched labels).
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct UsersByTagSearch {
    pub user_id: String,
    pub score: usize,
}

impl From<(String, f64)> for UsersByTagSearch {
    fn from(tuple: (String, f64)) -> Self {
        UsersByTagSearch {
            user_id: tuple.0,
            score: tuple.1 as usize,
        }
    }
}

impl RedisOps for UsersByTagSearch {}

impl UsersByTagSearch {
    /// Indexes user profile tags into per-label global sorted sets from a
    /// graph snapshot. Only for full rebuilds over quiescent or empty Redis
    /// (`db mock`, full reindex); live backfills go through
    /// [`Self::backfill_from_graph`] instead, which cannot overwrite
    /// concurrent watcher writes with snapshot state.
    ///
    /// # Errors
    /// Returns an error when the graph read or a Redis write fails.
    pub async fn reindex() -> ModelResult<()> {
        let rows = fetch_all_rows_from_graph(queries::get::global_tags_by_user()).await?;

        for row in rows {
            let label: &str = row.get("label").unwrap_or("");
            let sorted_set: Vec<(f64, &str)> = row.get("sorted_set").unwrap_or(Vec::new());
            if !label.is_empty() && !sorted_set.is_empty() {
                let key_parts = [&TAG_GLOBAL_USER_TAGGERS[..], &[label]].concat();
                Self::put_index_sorted_set(&key_parts, &sorted_set, None, None).await?;
            }
        }
        Ok(())
    }

    /// Searches users by profile tag labels with union semantics: any user
    /// whose profile carries at least one of the labels is returned, scored
    /// by the summed distinct-tagger counts, descending. Tie order between
    /// equal scores is unspecified.
    ///
    /// A single label is served from the per-label Redis sorted set; two or
    /// more labels aggregate in the graph under a 10-second budget.
    ///
    /// # Errors
    /// Returns [`crate::models::error::ModelError::KvOperationFailed`] on
    /// Redis failures and
    /// [`crate::models::error::ModelError::GraphOperationFailed`] on graph
    /// failures, including `GraphError::QueryTimeout` when the graph path
    /// exceeds its budget.
    pub async fn get_by_labels(
        labels: &[String],
        pagination: Pagination,
    ) -> ModelResult<Vec<UsersByTagSearch>> {
        match labels {
            // Single label: served straight from the per-label sorted set
            [label] => {
                let users = Self::try_from_index_sorted_set(
                    &[&TAG_GLOBAL_USER_TAGGERS[..], &[label.as_str()]].concat(),
                    None,
                    None,
                    pagination.skip,
                    pagination.limit,
                    SortOrder::Descending,
                    None,
                )
                .await?;
                Ok(users
                    .map(|list| list.into_iter().map(Into::into).collect())
                    .unwrap_or_default())
            }
            // Union across labels needs an aggregation over multiple sorted sets,
            // so it goes to the graph instead
            _ => Self::get_from_graph(labels, pagination.skip, pagination.limit).await,
        }
    }

    async fn get_from_graph(
        labels: &[String],
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> ModelResult<Vec<UsersByTagSearch>> {
        let graph = get_neo4j_graph()?;
        let query = queries::get::search_users_by_tags(labels, skip, limit);

        // The 10-second budget covers execution AND row streaming: execute()
        // only submits the query and the heavy work (ORDER BY materializes at
        // the first pull) happens while streaming, so a timeout on execute
        // alone lets a slow query run until the HTTP layer's 408.
        let users = timeout(Duration::from_secs(10), async {
            let mut result = graph.execute(query).await?;

            let mut users = Vec::new();
            while let Some(row) = result.try_next().await? {
                let user_id: String = row.get("user_id")?;
                let score: i64 = row.get("score")?;
                users.push(UsersByTagSearch {
                    user_id,
                    score: score as usize,
                });
            }
            Ok::<_, GraphError>(users)
        })
        .await
        .map_err(|_| GraphError::QueryTimeout)??;

        Ok(users)
    }

    /// Syncs the per-label score for a user from the taggers set the tag
    /// handlers already maintain idempotently: the score becomes the set
    /// cardinality, and the member is removed when the set empties or when
    /// the user's details carry the deleted sentinel. Everything runs as one
    /// Lua script, so concurrent events for the same (user, label) cannot
    /// commit a stale score and a concurrent tombstone cannot be raced; the
    /// callers need no gating and retries converge on their own. Must run
    /// after the taggers SADD/SREM settled.
    ///
    /// # Errors
    /// Returns an error if the Redis script execution fails.
    pub async fn sync_index_score(user_id: &str, label: &str) -> RedisResult<()> {
        let details_key = format!("{}:{}", UserDetails::prefix().await, user_id);
        Self::put_cardinality_index_sorted_set(
            &TagUser::prefix().await,
            &[user_id, label],
            &[&TAG_GLOBAL_USER_TAGGERS[..], &[label]].concat(),
            user_id,
            Some((&details_key, "$.name", USER_DELETED_SENTINEL)),
        )
        .await
    }

    /// Removes a tombstoned user from every per-label sorted set their
    /// profile tags placed them in. Each per-label sync re-checks the
    /// tombstone atomically, so a concurrent tag event cannot re-add them.
    ///
    /// # Errors
    /// Returns an error when reading the user's tag labels or syncing a
    /// label fails.
    pub async fn evict_user(user_id: &str) -> RedisResult<()> {
        let key_parts = [&USER_TAGS_KEY_PARTS[..], &[user_id]].concat();
        let mut skip = 0;
        loop {
            let page = Self::try_from_index_sorted_set(
                &key_parts,
                None,
                None,
                Some(skip),
                Some(EVICT_PAGE_SIZE),
                SortOrder::Descending,
                None,
            )
            .await?;
            let Some(page) = page else { break };
            if page.is_empty() {
                break;
            }
            let page_len = page.len();
            for (label, _) in page {
                Self::sync_index_score(user_id, &label).await?;
            }
            if page_len < EVICT_PAGE_SIZE {
                break;
            }
            skip += page_len;
        }
        Ok(())
    }

    /// Backfills the index against live data: the graph only enumerates
    /// candidate (user, label) pairs, and every score is derived atomically
    /// from the current taggers set by [`Self::sync_index_score`]. Events
    /// landing during the backfill are therefore never overwritten with
    /// snapshot state; a pair whose tag was deleted mid-backfill derives an
    /// empty set and stays out of the index.
    ///
    /// # Errors
    /// Returns an error when the graph enumeration or a Redis sync fails.
    pub async fn backfill_from_graph() -> ModelResult<()> {
        let rows = fetch_all_rows_from_graph(queries::get::get_user_tag_pairs()).await?;
        for row in rows {
            let user_id: String = row.get("user_id")?;
            let label: String = row.get("label")?;
            Self::sync_index_score(&user_id, &label).await?;
        }
        Ok(())
    }
}

/// List of user IDs
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct UserSearch(pub Vec<String>);

impl RedisOps for UserSearch {}

impl UserSearch {
    pub async fn get_by_name(
        name_prefix: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> RedisResult<Option<Self>> {
        // Perform the lexicographical range search
        let elements = Self::get_from_index_name(name_prefix, skip, limit).await?;

        // If elements exist, process them to extract user_ids
        if let Some(elements) = elements {
            let user_ids: Vec<String> = elements
                .into_iter()
                .filter_map(|element| {
                    // Split by `:` and take the last part (user_id)
                    element.split(':').next_back().map(|p| p.to_string())
                })
                .collect();

            return Ok(Some(UserSearch(user_ids)));
        }

        Ok(None)
    }

    pub async fn get_by_id(
        id_prefix: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> RedisResult<Option<Self>> {
        // Perform the lexicographical range search
        let elements = Self::get_from_index_id(id_prefix, skip, limit).await?;

        Ok(elements.map(UserSearch))
    }

    pub async fn get_from_index_name(
        name_prefix: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> RedisResult<Option<Vec<String>>> {
        // Convert the username to lowercase to ensure case-insensitive search
        let name_prefix = name_prefix.to_lowercase();

        let min = format!("[{name_prefix}"); // Inclusive range starting with "name_prefix"
        let max = format!("({name_prefix}~"); // Exclusive range ending just after "name_prefix"

        // Perform the lexicographical range search
        Self::try_from_index_sorted_set_lex(&USER_NAME_KEY_PARTS, &min, &max, skip, limit).await
    }

    pub async fn get_from_index_id(
        id_prefix: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> RedisResult<Option<Vec<String>>> {
        let id_prefix = id_prefix.to_lowercase();

        let min = format!("[{id_prefix}"); // Inclusive range starting with "id_prefix"
        let max = format!("({id_prefix}~"); // Exclusive range ending just after "id_prefix"

        Self::try_from_index_sorted_set_lex(&USER_ID_KEY_PARTS, &min, &max, skip, limit).await
    }

    /// Adds multiple `user_id`s to Redis sorted sets:
    /// - using the username as index
    /// - using the user ID as index
    ///
    /// This method takes a list of `UserDetails` and adds them all to the sorted set at once.
    pub async fn put_to_index(details_list: &[&UserDetails]) -> RedisResult<()> {
        // ensure existing records are deleted
        Self::delete_existing_records(
            details_list
                .iter()
                .map(|details| details.id.as_ref())
                .collect::<Vec<&str>>()
                .as_slice(),
        )
        .await?;

        // Collect all the `username:user_id` pairs
        let mut pairs: Vec<String> = Vec::with_capacity(details_list.len());
        let mut ids: Vec<String> = Vec::with_capacity(details_list.len());

        for details in details_list
            .iter()
            .filter(|d| d.name != USER_DELETED_SENTINEL)
        {
            // Convert the username to lowercase before storing
            let username = details.name.to_lowercase();
            let user_id = &details.id;

            pairs.push(format!("{username}:{user_id}"));
            ids.push(user_id.to_string());
        }

        let pairs_zscore_tuples = create_zero_score_tuples(&pairs);
        Self::put_index_sorted_set(&USER_NAME_KEY_PARTS, &pairs_zscore_tuples, None, None).await?;
        let ids_zscore_tuples = create_zero_score_tuples(&ids);
        Self::put_index_sorted_set(&USER_ID_KEY_PARTS, &ids_zscore_tuples, None, None).await
    }

    pub async fn delete(user_id: &str) -> RedisResult<()> {
        Self::delete_existing_records(&[user_id]).await
    }

    async fn delete_existing_records(user_ids: &[&str]) -> RedisResult<()> {
        if user_ids.is_empty() {
            return Ok(());
        }
        let mut records_to_delete: Vec<String> = Vec::with_capacity(user_ids.len());
        let keys: Vec<Vec<&str>> = user_ids.iter().map(|&id| vec![id]).collect();
        let users = UserDetails::get_from_index(keys.iter().map(|item| item.as_slice()).collect())
            .await?
            .into_iter()
            .flatten()
            .collect::<Vec<UserDetails>>();
        for user_id in user_ids {
            let existing_username = users
                .iter()
                .find(|user| user.id.to_string() == *user_id)
                .map(|user| user.name.to_lowercase());
            if let Some(existing_record) = existing_username {
                let search_key = format!("{existing_record}:{user_id}");
                records_to_delete.push(search_key);
            }
        }

        Self::remove_from_index_sorted_set(
            None,
            &USER_NAME_KEY_PARTS,
            &records_to_delete
                .iter()
                .map(|item| item.as_str())
                .collect::<Vec<&str>>(),
        )
        .await?;
        Self::remove_from_index_sorted_set(None, &USER_ID_KEY_PARTS, user_ids).await
    }
}
