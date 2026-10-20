use super::UserDetails;
use crate::db::graph::Query;
use crate::db::kv::{sorted_key, RedisResult, SortOrder};
use crate::db::{
    fetch_all_rows_from_graph, get_neo4j_graph, get_redis_conn, queries, GraphError, GraphResult,
    RedisOps,
};
use crate::models::error::ModelResult;
use crate::models::tag::user::{TagUser, USER_TAGS_KEY_PARTS};
use crate::types::{Pagination, StreamReach};
use futures::TryStreamExt;
use redis::Script;
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};
use utoipa::ToSchema;

pub const USER_NAME_KEY_PARTS: [&str; 2] = ["Users", "Name"];
pub const USER_ID_KEY_PARTS: [&str; 2] = ["Users", "ID"];
pub const TAG_GLOBAL_USER_TAGGERS: [&str; 4] = ["Tags", "Global", "User", "Taggers"];
/// `User:Search:Names`, a hash of user id to the lowercase name the user is
/// indexed under in `Sorted:Users:Name`. It is the only record of which
/// `name:id` member belongs to a user, so a rename, a delete and a retry can
/// remove it without reading the profile.
const SEARCH_NAMES_KEY_PARTS: [&str; 1] = ["Names"];
/// `User:Search:Deleted`, the set of tombstoned user ids: the Redis mirror of
/// the graph's `deleted` flag, read atomically by the users-by-tag sync.
pub const SEARCH_DELETED_KEY_PARTS: [&str; 1] = ["Deleted"];
const EVICT_PAGE_SIZE: usize = 500;
const REINDEX_PAGE_SIZE: usize = 1_000;

/// Converges one or more users' search state in one atomic step. For each
/// `(id, name, tombstoned)` triple: the member recorded in the names hash is
/// removed from the name index when it differs from `name`; an empty `name`
/// unindexes the user, any other indexes them under it; `tombstoned` (`1`)
/// adds the id to the deleted set, anything else removes it.
const SYNC_SEARCH_STATE: &str = r#"
    local names, by_name, by_id, deleted = KEYS[1], KEYS[2], KEYS[3], KEYS[4]
    for i = 1, #ARGV, 3 do
        local id, name, tombstoned = ARGV[i], ARGV[i + 1], ARGV[i + 2]
        local previous = redis.call('HGET', names, id)
        if previous and previous ~= name then
            redis.call('ZREM', by_name, previous .. ':' .. id)
        end
        if name == '' then
            redis.call('HDEL', names, id)
            redis.call('ZREM', by_id, id)
        else
            redis.call('HSET', names, id, name)
            redis.call('ZADD', by_name, 0, name .. ':' .. id)
            redis.call('ZADD', by_id, 0, id)
        end
        if tombstoned == '1' then
            redis.call('SADD', deleted, id)
        else
            redis.call('SREM', deleted, id)
        end
    end
    return #ARGV / 3
"#;

/// The [`SYNC_SEARCH_STATE`] triple for a user: indexed under their lowercase
/// name, or under none when tombstoned or without a `uri`.
fn search_entry(details: &UserDetails, without_uri: bool) -> (&str, String, bool) {
    let name = match details.deleted || without_uri {
        true => String::new(),
        false => details.name.to_lowercase(),
    };
    (details.id.as_ref(), name, details.deleted)
}

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
    /// graph snapshot, streaming one label row at a time. Only for full
    /// rebuilds over quiescent or empty Redis (`db mock`, full reindex); live
    /// backfills go through the `UsersByTagsIndexBackfill` migration instead,
    /// which derives every score from live state and cannot overwrite
    /// concurrent watcher writes.
    ///
    /// # Errors
    /// Returns an error when the graph read or a Redis write fails.
    pub async fn reindex() -> ModelResult<()> {
        let graph = get_neo4j_graph()?;
        let mut rows = graph
            .execute(queries::get::global_tags_by_user())
            .await
            .map_err(GraphError::from)?;

        while let Some(row) = rows.try_next().await.map_err(GraphError::from)? {
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
    /// by the summed distinct-tagger counts, descending. Equal scores break
    /// ties by user id descending on both serving paths.
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
            _ => Self::get_from_graph(queries::get::search_users_by_tags(
                labels,
                pagination.skip,
                pagination.limit,
            ))
            .await
            .map_err(Into::into),
        }
    }

    /// [`Self::get_by_labels`] restricted to the users in `user_id`'s
    /// `reach`, excluding `user_id`. Scores are the same as unfiltered, and an
    /// unknown user yields an empty list. Always served from the graph, which
    /// pages the result itself, under the same 10-second budget.
    ///
    /// # Errors
    /// Returns [`crate::models::error::ModelError::GraphOperationFailed`] on
    /// graph failures, including `GraphError::QueryTimeout`.
    pub async fn get_by_labels_with_reach(
        labels: &[String],
        user_id: &str,
        reach: StreamReach,
        pagination: Pagination,
    ) -> ModelResult<Vec<UsersByTagSearch>> {
        Self::get_from_graph(queries::get::search_users_by_tags_with_reach(
            labels,
            user_id,
            &reach,
            pagination.skip,
            pagination.limit,
        ))
        .await
        .map_err(Into::into)
    }

    async fn get_from_graph(query: Query) -> GraphResult<Vec<UsersByTagSearch>> {
        let graph = get_neo4j_graph()?;

        // The 10-second budget covers execution AND row streaming: execute()
        // only submits the query and the heavy work (ORDER BY materializes at
        // the first pull) happens while streaming, so a timeout on execute
        // alone lets a slow query run until the HTTP layer's 408.
        timeout(Duration::from_secs(10), async {
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
            Ok(users)
        })
        .await
        .map_err(|_| GraphError::QueryTimeout)?
    }

    /// Syncs the per-label score for a user from the taggers set the tag
    /// handlers already maintain idempotently: the score becomes the set
    /// cardinality, and the member is removed when the set empties or when
    /// the user is in the deleted set [`UserSearch`] maintains. Everything
    /// runs as one Lua script, so concurrent events for the same (user, label)
    /// cannot commit a stale score and a concurrent tombstone cannot be
    /// raced; the callers need no gating and retries converge on their own.
    /// Must run after the taggers SADD/SREM settled.
    ///
    /// # Errors
    /// Returns an error if the Redis script execution fails.
    pub async fn sync_index_score(user_id: &str, label: &str) -> RedisResult<()> {
        let deleted_key = UserSearch::deleted_set_key().await;
        Self::put_cardinality_index_sorted_set(
            &TagUser::prefix().await,
            &[user_id, label],
            &[&TAG_GLOBAL_USER_TAGGERS[..], &[label]].concat(),
            user_id,
            Some((&deleted_key, user_id)),
        )
        .await
    }

    /// Re-derives the user's entry in every per-label sorted set their
    /// profile tags placed them in: evicts them while they are in the deleted
    /// set, restores them once a recreated profile removed them from it. Each
    /// per-label sync checks the set atomically, so concurrent tag events
    /// cannot race the outcome. Must run after the [`UserSearch`] write that
    /// changed the set settled.
    ///
    /// # Errors
    /// Returns an error when reading the user's tag labels or syncing a
    /// label fails.
    pub async fn sync_user_labels(user_id: &str) -> RedisResult<()> {
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

    /// Converges each user's search state with their profile, atomically per
    /// call: a live user is indexed under their lowercase name and by id, and
    /// leaves the deleted set; a tombstone leaves both indexes and joins it.
    /// The member a rename or a delete has to drop comes from the names hash,
    /// so the call is safe to retry after any partial failure and never reads
    /// the profile.
    pub async fn put_to_index(details_list: &[&UserDetails]) -> RedisResult<()> {
        let entries: Vec<(&str, String, bool)> = details_list
            .iter()
            .map(|details| search_entry(details, false))
            .collect();
        Self::sync_state(&entries).await
    }

    /// Drops a hard-deleted user from both indexes and from the deleted set.
    pub async fn delete(user_id: &str) -> RedisResult<()> {
        Self::sync_state(&[(user_id, String::new(), false)]).await
    }

    /// Rebuilds the search state of every user from the graph, one keyset page
    /// at a time, and evicts each tombstone from the users-by-tag index. A
    /// user without a `uri` leaves the search, as only a profile write stores
    /// one: the rebuild drops whatever an earlier pass indexed them under. That
    /// covers the discovery stubs, and a profile indexed before uris were
    /// stored and missing from the event log, until its next profile write.
    ///
    /// Run with the watcher stopped: a profile event between a page's read and
    /// its write is overwritten with the page's older state. Existing state is
    /// fine: the names hash drops the member of a user renamed since the last
    /// sync.
    ///
    /// # Errors
    /// Returns an error when a graph page or a Redis write fails.
    pub async fn reindex() -> ModelResult<()> {
        let mut after_id = String::new();
        while let Some(last_id) = Self::reindex_page(&after_id, REINDEX_PAGE_SIZE).await? {
            after_id = last_id;
        }
        Ok(())
    }

    /// Rebuilds the search state of the users on the keyset page after
    /// `after_id`, and returns the page's last id: `None` past the last page.
    async fn reindex_page(after_id: &str, limit: usize) -> ModelResult<Option<String>> {
        let rows = fetch_all_rows_from_graph(queries::get::get_users_page(after_id, limit)).await?;
        let Some(last_id) = rows.last().and_then(|row| row.get::<String>("id").ok()) else {
            return Ok(None);
        };

        let mut page = Vec::with_capacity(rows.len());
        for row in &rows {
            let without_uri: bool = row.get("without_uri").map_err(GraphError::from)?;
            if let Some(details) = UserDetails::from_row(row) {
                page.push((details, without_uri));
            }
        }
        let entries: Vec<(&str, String, bool)> = page
            .iter()
            .map(|(details, without_uri)| search_entry(details, *without_uri))
            .collect();
        Self::sync_state(&entries).await?;
        for (tombstone, _) in page.iter().filter(|(details, _)| details.deleted) {
            UsersByTagSearch::sync_user_labels(&tombstone.id).await?;
        }
        Ok(Some(last_id))
    }

    /// The lowercase name each user is indexed under, `None` when unindexed.
    pub async fn indexed_names(user_ids: &[&str]) -> RedisResult<Vec<Option<String>>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut conn = get_redis_conn().await?;
        Ok(redis::cmd("HMGET")
            .arg(Self::names_hash_key().await)
            .arg(user_ids)
            .query_async(&mut conn)
            .await?)
    }

    /// Full key of the deleted set, for guards that run inside another script.
    pub async fn deleted_set_key() -> String {
        format!(
            "{}:{}",
            Self::prefix().await,
            SEARCH_DELETED_KEY_PARTS.join(":")
        )
    }

    async fn names_hash_key() -> String {
        format!(
            "{}:{}",
            Self::prefix().await,
            SEARCH_NAMES_KEY_PARTS.join(":")
        )
    }

    async fn sync_state(entries: &[(&str, String, bool)]) -> RedisResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let script = Script::new(SYNC_SEARCH_STATE);
        let mut invocation = script.prepare_invoke();
        invocation
            .key(Self::names_hash_key().await)
            .key(sorted_key(&USER_NAME_KEY_PARTS))
            .key(sorted_key(&USER_ID_KEY_PARTS))
            .key(Self::deleted_set_key().await);
        for (user_id, name, tombstoned) in entries {
            invocation
                .arg(*user_id)
                .arg(name)
                .arg(if *tombstoned { "1" } else { "0" });
        }
        let mut conn = get_redis_conn().await?;
        let _: i64 = invocation.invoke_async(&mut conn).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::exec_single_row;
    use crate::utils::test_utils::random_pubky_id;
    use crate::{types::DynError, StackConfig, StackManager};

    /// Rebuilds only the page that starts at `user_id`: no other id falls
    /// between it and its prefix one character shorter.
    async fn reindex_one(user_id: &str) -> ModelResult<()> {
        UserSearch::reindex_page(&user_id[..user_id.len() - 1], 1).await?;
        Ok(())
    }

    /// The name the hash records for `user_id`, and whether the name index
    /// holds them under `name` and the id index holds them at all.
    async fn search_state(
        user_id: &str,
        name: &str,
    ) -> Result<(Option<String>, bool, bool), DynError> {
        let recorded = UserSearch::indexed_names(&[user_id]).await?.pop().flatten();
        let name_member = format!("{name}:{user_id}");
        let by_name =
            UserSearch::check_sorted_set_member(None, &USER_NAME_KEY_PARTS, &[&name_member])
                .await?;
        let by_id =
            UserSearch::check_sorted_set_member(None, &USER_ID_KEY_PARTS, &[user_id]).await?;
        Ok((recorded, by_name.is_some(), by_id.is_some()))
    }

    /// The rebuild decides by the `uri` alone, not by the name: a stub and a
    /// profile indexed before uris were stored both lose what an earlier pass
    /// indexed them under, as `UsersByPkReindex` indexes every node, and a
    /// profile named after its key stays indexed because it has a `uri`.
    #[tokio_shared_rt::test(shared)]
    async fn test_reindex_keeps_only_users_with_a_uri() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let stub = UserDetails::from_pubky(random_pubky_id());
        let mut legacy = UserDetails::from_pubky(random_pubky_id());
        legacy.name = "Legacy Profile".to_string();
        let profile = UserDetails::from_pubky(random_pubky_id());
        let ids = [&stub, &legacy, &profile].map(|details| details.id.to_string());
        let [stub_id, legacy_id, profile_id] = &ids;

        for without_uri in [&stub, &legacy] {
            without_uri.put_to_graph().await?;
        }
        UserSearch::put_to_index(&[&stub, &legacy]).await?;
        profile
            .put_profile_to_graph(&format!("pubky://{profile_id}/pub/pubky.app/profile.json"))
            .await?;

        let outcome = async {
            for user_id in &ids {
                reindex_one(user_id).await?;
            }
            assert_eq!(
                search_state(stub_id, stub_id).await?,
                (None, false, false),
                "the stub leaves the search"
            );
            assert_eq!(
                search_state(legacy_id, "legacy profile").await?,
                (None, false, false),
                "a profile without a uri leaves the search"
            );
            assert_eq!(
                search_state(profile_id, profile_id).await?,
                (Some(profile_id.clone()), true, true),
                "the profile stays indexed under its name"
            );
            Ok::<(), DynError>(())
        }
        .await;

        exec_single_row(
            Query::new(
                "user_search_test_cleanup",
                "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
            )
            .param("ids", ids.to_vec()),
        )
        .await?;
        for user_id in &ids {
            UserSearch::delete(user_id).await?;
        }
        outcome
    }
}
