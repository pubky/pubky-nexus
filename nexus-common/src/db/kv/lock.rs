//! Redis primitives for a TTL-based distributed lock. The TTL frees the lock if
//! a holder dies without releasing it.

use crate::db::get_redis_conn;
use crate::db::kv::error::RedisResult;
use deadpool_redis::redis::{self, Script};
use std::sync::LazyLock;

/// Compare-and-delete: release only if the token still matches, so a re-taken
/// lease isn't dropped by the previous holder.
static RELEASE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r"if redis.call('get', KEYS[1]) == ARGV[1] then
            return redis.call('del', KEYS[1])
        else
            return 0
        end",
    )
});

/// Tries to claim `key` with `token` for `ttl_secs`. `Ok(false)` when another
/// holder has it. `SET NX EX` acquires and arms the expiry atomically.
pub async fn try_acquire_lock(key: &str, token: &str, ttl_secs: u64) -> RedisResult<bool> {
    let mut conn = get_redis_conn().await?;
    // SET returns Some when NX succeeds, None when the key exists.
    let acquired: Option<String> = redis::cmd("SET")
        .arg(key)
        .arg(token)
        .arg("NX")
        .arg("EX")
        .arg(ttl_secs)
        .query_async(&mut conn)
        .await?;
    Ok(acquired.is_some())
}

/// Releases `key` only if still held by `token` (see [`RELEASE`]).
pub async fn release_lock(key: &str, token: &str) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;
    let _: i64 = RELEASE.key(key).arg(token).invoke_async(&mut conn).await?;
    Ok(())
}
