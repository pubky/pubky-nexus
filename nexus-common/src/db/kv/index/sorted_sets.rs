use crate::db::get_redis_conn;
use crate::db::kv::{RedisError, RedisResult};
use redis::{AsyncCommands, Script};
use serde::Deserialize;
use std::sync::LazyLock;
use utoipa::ToSchema;

#[derive(Clone, Deserialize, Debug, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    Ascending,
    #[default]
    Descending,
}

pub enum ScoreAction {
    Increment(f64),
    Decrement(f64),
}

pub const SORTED_PREFIX: &str = "Sorted";

/// Checks if a member exists in a Redis sorted set and retrieves its score.
///
/// This function checks whether a specified member exists in a Redis sorted set
/// by retrieving its score using the `ZSCORE` command. If the member is present,
/// its score is returned; if it is not present, `None` is returned.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis key.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `member` - A string slice representing the member to check in the sorted set.
///
/// # Returns
///
/// Returns an `Option<isize>` containing the score of the member if it exists, or `None` if it does not.
pub async fn check_member(prefix: &str, key: &str, member: &str) -> RedisResult<Option<isize>> {
    let index_key = format!("{prefix}:{key}");
    let mut redis_conn = get_redis_conn().await?;
    // Direct ZSCORE instead of check_members() to avoid pipeline overhead for a single call.
    let rank = redis_conn.zscore(index_key, member).await?;
    Ok(rank)
}

/// Checks multiple (key, member) pairs in Redis sorted sets using a single
/// pipeline of `ZSCORE` commands.
///
/// Each `(key, member)` pair produces one `ZSCORE` call. Returns scores in
/// the same order, with `None` for absent members.
pub async fn check_members(
    prefix: &str,
    pairs: &[(&str, &str)],
) -> RedisResult<Vec<Option<isize>>> {
    if pairs.is_empty() {
        return Ok(Vec::new());
    }

    let mut pipe = redis::pipe();
    for (key, member) in pairs {
        let index_key = format!("{prefix}:{key}");
        pipe.zscore(index_key, *member);
    }

    let mut redis_conn = get_redis_conn().await?;
    let results: Vec<Option<isize>> = pipe.query_async(&mut redis_conn).await?;
    Ok(results)
}

/// Adds elements to a Redis sorted set.
///
/// This function adds elements to the specified Redis sorted set. If the set doesn't exist,
/// it creates a new sorted set.
///
/// # Argumentsf64
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `values` - A slice of tuples where each tuple contains a reference to a string slice representing the element and a f64 representing the score of the element.
/// * `expiration` - An optional `i64` specifying the TTL (in seconds) for the set. If `None`, no TTL will be set.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn put(
    prefix: &str,
    key: &str,
    items: &[(f64, &str)],
    expiration: Option<i64>,
) -> RedisResult<()> {
    if items.is_empty() {
        return Ok(());
    }

    let index_key = format!("{prefix}:{key}");
    let mut redis_conn = get_redis_conn().await?;

    let mut pipe = redis::pipe();

    pipe.zadd_multiple(&index_key, items);

    if let Some(ttl) = expiration {
        // TTL convert to seconds
        pipe.expire(&index_key, ttl);
    }

    let _: () = pipe.query_async(&mut redis_conn).await?;
    Ok(())
}

/// Seeds `member` with `score` only if it is not already in the sorted set.
///
/// `ZADD NX` makes the check and the write one atomic operation, so an existing
/// member keeps its score whatever concurrent writers are doing.
pub async fn add_member_if_absent(
    prefix: &str,
    key: &str,
    score: f64,
    member: &str,
) -> RedisResult<()> {
    let index_key = format!("{prefix}:{key}");
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis::cmd("ZADD")
        .arg(index_key)
        .arg("NX")
        .arg(score)
        .arg(member)
        .query_async(&mut redis_conn)
        .await?;
    Ok(())
}

/// Updates the score of a member in a Redis sorted set.
///
/// This function modifies the score of a member in the specified Redis sorted set by incrementing or decrementing it
/// based on the provided `ScoreAction`.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `member` - A string slice representing the member whose score will be updated.
/// * `score_mutation` - A `ScoreAction` that indicates whether to increment or decrement the score.
pub async fn put_score(
    prefix: &str,
    key: &str,
    member: &str,
    score_mutation: ScoreAction,
) -> RedisResult<()> {
    let index_key = format!("{prefix}:{key}");
    let mut redis_conn = get_redis_conn().await?;
    let value = match score_mutation {
        ScoreAction::Increment(val) => val,
        ScoreAction::Decrement(val) => -val,
    };
    let _: () = redis_conn.zincr(&index_key, member, value).await?;

    Ok(())
}

/// Retrieves a range of elements from a Redis sorted set.
///
/// This function retrieves elements from a specified Redis sorted set based on a score range.
/// The range is defined by `min_score` and `max_score` parameters, where `min_score` and `max_score`
/// specify the inclusive lower and upper bounds of the scores.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `min_score` - The minimum score for the range (inclusive).
/// * `max_score` - The maximum score for the range (inclusive).
/// * `skip` - An optional number of elements to skip (useful for pagination).
/// * `limit` - The maximum number of elements to retrieve.
/// * `sorting` - The sorting order (ascending or descending).
///
/// # Returns
///
/// Returns a vector of tuples containing the elements and their scores.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn get_range(
    prefix: &str,
    key: &str,
    min_score: Option<f64>,
    max_score: Option<f64>,
    skip: Option<usize>,
    limit: Option<usize>,
    sorting: SortOrder,
) -> RedisResult<Option<Vec<(String, f64)>>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{prefix}:{key}");

    // Make sure if the key that we want to find, it is in the sorted set
    if !redis_conn.exists(&index_key).await? {
        return Ok(None);
    }

    let min_score = min_score.unwrap_or(f64::MIN);
    let max_score = max_score.unwrap_or(f64::MAX);
    let skip = skip.unwrap_or(0) as isize;
    let limit = limit.unwrap_or(1000) as isize;

    // ZRANGE with the WITHSCORES option retrieves both: the elements and their scores
    let elements: Vec<(String, f64)> = match sorting {
        SortOrder::Ascending => {
            redis_conn
                .zrangebyscore_limit_withscores(index_key, min_score, max_score, skip, limit)
                .await?
        }
        SortOrder::Descending => {
            redis_conn
                .zrevrangebyscore_limit_withscores(index_key, max_score, min_score, skip, limit)
                .await?
        }
    };
    Ok(Some(elements))
}

/// Performs a lexicographical range search on the Redis sorted set.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `min` - The minimum lexicographical bound (inclusive).
/// * `max` - The maximum lexicographical bound (exclusive).
/// * `skip` - An optional number of elements to skip (useful for pagination).
/// * `limit` - The maximum number of elements to retrieve.
pub async fn get_lex_range(
    prefix: &str,
    key: &str,
    min: &str,
    max: &str,
    skip: Option<usize>,
    limit: Option<usize>,
) -> RedisResult<Option<Vec<String>>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{prefix}:{key}");
    let skip = skip.unwrap_or(0) as isize;
    let limit = limit.unwrap_or(1000) as isize;

    let elements: Vec<String> = redis_conn
        .zrangebylex_limit(index_key, min, max, skip, limit)
        .await?;

    match elements.len() {
        0 => Ok(None),
        _ => Ok(Some(elements)),
    }
}

/// Removes elements from the Redis sorted set.
///
/// # Arguments
///
/// * `items` - A slice of elements to remove.
pub async fn _remove(prefix: &str, key: &str, items: &[&str]) -> RedisResult<()> {
    if items.is_empty() {
        return Ok(());
    }

    let index_key = format!("{prefix}:{key}");
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn.zrem(&index_key, items).await?;
    Ok(())
}

/// Removes elements from a Redis sorted set.
///
/// This function removes the specified elements from the Redis sorted set identified by the `prefix` and `key`.
/// If the sorted set does not exist, it will simply return without error.
///
/// # Arguments
///
/// * `prefix` - A string slice representing the prefix for the Redis keys.
/// * `key` - A string slice representing the key under which the sorted set is stored.
/// * `values` - A slice of string slices representing the elements to be removed from the sorted set.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn del(prefix: &str, key: &str, values: &[&str]) -> RedisResult<()> {
    if values.is_empty() {
        return Ok(());
    }

    let index_key = format!("{prefix}:{key}");
    let mut redis_conn = get_redis_conn().await?;

    // Remove the elements from the sorted set
    let _: () = redis_conn.zrem(index_key, values).await?;
    Ok(())
}

/// Lua script for atomically replacing a sorted set: DEL + ZADD + (optional) EXPIRE.
///
/// EVAL is atomic server-side and a single round trip, so a cancelled call cannot
/// leave client-side transaction state on the pooled connection — the same hazard
/// that MULTI/EXEC suffers when the scheduler cancels the job future.
///
/// ARGV[1] is the TTL in seconds (0 means no expiry); ARGV[2..] are alternating
/// score, member pairs passed to ZADD.
static REPLACE_SORTED_SET: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r"redis.call('del', KEYS[1])
          if #ARGV > 1 then
              redis.call('zadd', KEYS[1], unpack(ARGV, 2))
          end
          if tonumber(ARGV[1]) > 0 then
              redis.call('expire', KEYS[1], tonumber(ARGV[1]))
          end
          return 1",
    )
});

/// Maximum number of members `replace` accepts in a single call.
///
/// Lua's `unpack` inside `REPLACE_SORTED_SET` is bounded by the Lua C stack
/// (LUAI_MAXCSTACK, ~8000 elements). ARGV[1] is the TTL and each member
/// contributes a score and a member string, so the script receives
/// `1 + 2 * members` arguments; 3999 members == 7999 args, within the limit.
const MAX_REPLACE_MEMBERS: usize = 3999;

/// Atomically replaces a sorted set: DEL + ZADD + (optional) EXPIRE in a single
/// Lua script so readers never observe an empty or half-built key.
///
/// When `items` is empty the key is deleted. This is a caller-selected behaviour:
/// the caller decides whether to invoke `replace` with an empty list (evict the key)
/// or to skip the call entirely (leave the previous value intact).
///
/// # Arguments
///
/// * `prefix` - Prefix for the Redis keys.
/// * `key` - Key under which the sorted set is stored.
/// * `items` - `(score, member)` pairs to write.
/// * `expiration` - Optional TTL in seconds.
///
/// # Errors
///
/// Returns `RedisError::InvalidInput` when `items` exceeds `MAX_REPLACE_MEMBERS`,
/// the bound imposed by Lua's `unpack` stack inside the script.
pub async fn replace(
    prefix: &str,
    key: &str,
    items: &[(f64, &str)],
    expiration: Option<i64>,
) -> RedisResult<()> {
    let index_key = format!("{prefix}:{key}");
    let mut redis_conn = get_redis_conn().await?;

    if items.is_empty() {
        let _: () = redis_conn.del(&index_key).await?;
        return Ok(());
    }

    // Enforced in release too: an oversized call must fail here with a clear
    // error, not inside the Lua script as an opaque `unpack` stack error.
    if items.len() > MAX_REPLACE_MEMBERS {
        return Err(RedisError::InvalidInput(format!(
            "replace: {} members exceeds the limit of {MAX_REPLACE_MEMBERS} \
             (Lua unpack stack: ARGV[1]=ttl + 2*members)",
            items.len(),
        )));
    }

    let ttl: i64 = expiration.unwrap_or(0);
    let mut args: Vec<String> = Vec::with_capacity(1 + items.len() * 2);
    args.push(ttl.to_string());
    for (score, member) in items {
        args.push(score.to_string());
        args.push(member.to_string());
    }

    let _: () = REPLACE_SORTED_SET
        .key(&index_key)
        .arg(&args)
        .invoke_async(&mut redis_conn)
        .await?;
    Ok(())
}

/// Returns the remaining TTL (in seconds) for a key.
///
/// Redis TTL returns `-2` when the key does not exist and `-1` when the key
/// exists but has no expiry. Both cases are mapped to `None`. A present key
/// with an expiry returns `Some(remaining_seconds)`.
///
/// # Arguments
///
/// * `prefix` - Prefix for the Redis keys.
/// * `key` - Key to check.
#[cfg(test)]
pub async fn ttl(prefix: &str, key: &str) -> RedisResult<Option<i64>> {
    let index_key = format!("{prefix}:{key}");
    let mut redis_conn = get_redis_conn().await?;
    let raw: i64 = redis_conn.ttl(&index_key).await?;
    match raw {
        -2 | -1 => Ok(None),
        n => Ok(Some(n)),
    }
}

/// Atomically derives a sorted-set member's score from a set's cardinality:
/// the member's score becomes `SCARD` of the source set, and the member is
/// removed when the set is empty.
///
/// The read and the conditional write run as one Lua script, so concurrent
/// writers cannot commit a stale cardinality between the two commands.
///
/// # Arguments
///
/// * `set_prefix` - Prefix of the source set key.
/// * `set_key` - Key of the source set whose cardinality becomes the score.
/// * `sorted_set_prefix` - Prefix of the destination sorted set key.
/// * `sorted_set_key` - Key of the destination sorted set.
/// * `member` - The sorted-set member to write or remove.
/// * `removal_guard` - Optional `(json_key, json_path, value)`: when the JSON
///   document at `json_key` holds `value` at `json_path`, the member is
///   removed regardless of cardinality. Checked inside the same script, so
///   the guard cannot race the write.
///
/// # Errors
///
/// Returns an error if the script execution fails.
pub async fn sync_score_from_set_cardinality(
    set_prefix: &str,
    set_key: &str,
    sorted_set_prefix: &str,
    sorted_set_key: &str,
    member: &str,
    removal_guard: Option<(&str, &str, &str)>,
) -> RedisResult<()> {
    let set_index_key = format!("{set_prefix}:{set_key}");
    let sorted_set_index_key = format!("{sorted_set_prefix}:{sorted_set_key}");
    let mut redis_conn = get_redis_conn().await?;

    let derive = r#"
        local cardinality = redis.call('SCARD', KEYS[1])
        if cardinality == 0 then
            redis.call('ZREM', KEYS[2], ARGV[1])
        else
            redis.call('ZADD', KEYS[2], cardinality, ARGV[1])
        end
        return cardinality
    "#;

    let invocation = match removal_guard {
        Some((json_key, json_path, value)) => {
            let script = Script::new(&format!(
                r#"
                local guarded = redis.call('JSON.GET', KEYS[3], ARGV[2])
                if guarded then
                    local decoded = cjson.decode(guarded)
                    if type(decoded) == 'table' and decoded[1] == ARGV[3] then
                        redis.call('ZREM', KEYS[2], ARGV[1])
                        return 0
                    end
                end
                {derive}
            "#
            ));
            script
                .key(set_index_key)
                .key(sorted_set_index_key)
                .key(json_key)
                .arg(member)
                .arg(json_path)
                .arg(value)
                .invoke_async(&mut redis_conn)
                .await
        }
        None => {
            let script = Script::new(derive);
            script
                .key(set_index_key)
                .key(sorted_set_index_key)
                .arg(member)
                .invoke_async(&mut redis_conn)
                .await
        }
    };

    let _: i64 = invocation?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{types::DynError, StackConfig, StackManager};

    const TEST_PREFIX: &str = "SortedSetReplaceTest";

    /// Each test owns its key: they share one Redis and run concurrently.
    #[tokio_shared_rt::test(shared)]
    async fn replace_evicts_members_missing_from_the_new_set() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let key = "evicts";
        put(
            TEST_PREFIX,
            key,
            &[(10.0, "a"), (20.0, "b"), (30.0, "c")],
            None,
        )
        .await?;

        replace(TEST_PREFIX, key, &[(5.0, "b"), (40.0, "d")], None).await?;

        assert_eq!(
            check_member(TEST_PREFIX, key, "b").await?,
            Some(5),
            "a member present in both sets must take the new score"
        );
        assert_eq!(check_member(TEST_PREFIX, key, "d").await?, Some(40));
        // An additive ZADD would leave these behind with their stale scores.
        for evicted in ["a", "c"] {
            assert_eq!(
                check_member(TEST_PREFIX, key, evicted).await?,
                None,
                "{evicted} dropped out of the new set and must not survive the replace"
            );
        }

        replace(TEST_PREFIX, key, &[], None).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn replace_arms_the_ttl() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let key = "arms-ttl";
        replace(TEST_PREFIX, key, &[(1.0, "a")], Some(60)).await?;

        let remaining = ttl(TEST_PREFIX, key)
            .await?
            .expect("a replace with an expiration must leave a TTL on the key");
        assert!(
            remaining > 0 && remaining <= 60,
            "TTL must sit inside the requested window, got {remaining}"
        );

        replace(TEST_PREFIX, key, &[], None).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn replace_without_items_deletes_the_key() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let key = "empty-clears";
        put(TEST_PREFIX, key, &[(1.0, "a")], Some(60)).await?;

        replace(TEST_PREFIX, key, &[], None).await?;

        assert_eq!(
            check_member(TEST_PREFIX, key, "a").await?,
            None,
            "an empty replace must not leave a stale set behind"
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn replace_rejects_more_members_than_lua_unpack_supports() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let key = "over-limit";
        let members: Vec<String> = (0..=MAX_REPLACE_MEMBERS).map(|i| format!("m{i}")).collect();
        let items: Vec<(f64, &str)> = members
            .iter()
            .enumerate()
            .map(|(i, m)| (i as f64, m.as_str()))
            .collect();

        let err = replace(TEST_PREFIX, key, &items, None)
            .await
            .expect_err("an oversized replace must be rejected with a typed error");
        assert!(
            matches!(err, RedisError::InvalidInput(_)),
            "expected RedisError::InvalidInput, got {err:?}"
        );
        assert_eq!(
            check_member(TEST_PREFIX, key, "m0").await?,
            None,
            "a rejected replace must not have written anything"
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn replace_accepts_exactly_the_member_limit() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let key = "at-limit";
        let members: Vec<String> = (0..MAX_REPLACE_MEMBERS).map(|i| format!("m{i}")).collect();
        let items: Vec<(f64, &str)> = members
            .iter()
            .enumerate()
            .map(|(i, m)| (i as f64, m.as_str()))
            .collect();

        replace(TEST_PREFIX, key, &items, None).await?;

        assert_eq!(check_member(TEST_PREFIX, key, "m0").await?, Some(0));
        assert_eq!(
            check_member(TEST_PREFIX, key, members.last().unwrap()).await?,
            Some((MAX_REPLACE_MEMBERS - 1) as isize),
            "the boundary write must land in full, proving the limit math"
        );

        replace(TEST_PREFIX, key, &[], None).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn replace_with_zero_ttl_leaves_no_expiry() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let key = "zero-ttl";
        // First write with a TTL so the key has an expiry.
        replace(TEST_PREFIX, key, &[(1.0, "a")], Some(60)).await?;
        assert!(
            ttl(TEST_PREFIX, key).await?.is_some(),
            "initial write must have a TTL"
        );

        // Overwrite with expiration Some(0) — the key must have no expiry.
        replace(TEST_PREFIX, key, &[(2.0, "b")], Some(0)).await?;
        assert_eq!(
            ttl(TEST_PREFIX, key).await?,
            None,
            "a replace with 0 TTL must leave the key with no expiry"
        );
        assert_eq!(
            check_member(TEST_PREFIX, key, "b").await?,
            Some(2),
            "the data must still be written"
        );

        replace(TEST_PREFIX, key, &[], None).await?;
        Ok(())
    }
}
