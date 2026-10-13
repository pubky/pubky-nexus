use std::sync::LazyLock;

use deadpool_redis::redis::Script;
use serde::{Deserialize, Serialize};

use crate::db::get_redis_conn;
use crate::db::kv::{cursor_backwards_error, RedisError, RedisResult, SORTED_PREFIX};
use crate::db::RedisOps;

const USER_HS_CURSOR: [&str; 2] = ["Users", "Homeservers"];

/// Script for [`UserHsCursor::write`]. KEYS[1] is the user's cursor sorted set, ARGV[1] the
/// new cursor and ARGV[2] the homeserver ID, its member. Returns -1 without writing when the
/// new cursor is below the stored one, else writes it and returns 1.
static WRITE_SCRIPT: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r"local stored = redis.call('ZSCORE', KEYS[1], ARGV[2])
          if stored and tonumber(ARGV[1]) < tonumber(stored) then
              return -1
          end
          redis.call('ZADD', KEYS[1], ARGV[1], ARGV[2])
          return 1",
    )
});

/// Redis key parts for per-user homeserver cursor storage: `["Users", "Homeservers", <user_id>]`.
pub type UserHsCursorKey<'a> = [&'a str; 3];

/// Builds the Redis key path for per-user homeserver cursor storage.
pub fn user_hs_cursor_key(user_id: &str) -> UserHsCursorKey<'_> {
    [USER_HS_CURSOR[0], USER_HS_CURSOR[1], user_id]
}

/// Per-user event cursor for a homeserver's event stream.
///
/// A marker type that owns the Redis read/write operations for cursors; it holds
/// no data, the cursor values live in Redis sorted sets. `Serialize`/`Deserialize`
/// are only present to satisfy the [`RedisOps`] trait bounds.
#[derive(Serialize, Deserialize)]
pub struct UserHsCursor;

#[async_trait::async_trait]
impl RedisOps for UserHsCursor {}

impl UserHsCursor {
    /// Batch-reads each user's stored event cursor for `hs_id`, returning `0`
    /// for users with no cursor entry yet (newly ingested).
    ///
    /// Each user's cursor lives in its own `USER_HS_CURSOR` sorted set (keyed by
    /// user ID) with the homeserver ID as the member; all lookups are batched
    /// into a single `check_sorted_set_members` pipeline call. Redis errors are
    /// propagated instead of silently rewinding to 0.
    ///
    /// The cursor is stored as the score (f64), exact for integer values up to
    /// 2^53 — practically unreachable for monotonic event IDs.
    pub async fn read(user_ids: &[&str], hs_id: &str) -> RedisResult<Vec<u64>> {
        let keys: Vec<UserHsCursorKey> = user_ids.iter().map(|u| user_hs_cursor_key(u)).collect();
        let pairs: Vec<(&[&str], &[&str])> = keys
            .iter()
            .map(|k| (k.as_slice(), std::slice::from_ref(&hs_id)))
            .collect();
        let scores = Self::check_sorted_set_members(None, &pairs).await?;
        scores
            .into_iter()
            .map(|s| {
                // Score is retrieved from Redis as isize, which can have negative values.
                // A negative score (due to manual intervention, or Redis corruption) would result in
                // a very high cursor value when converted to u64, meaning that user gets no more events
                let score = s.unwrap_or(0);
                if score < 0 {
                    let msg = format!("negative cursor score {score} for HS {hs_id}");
                    return Err(RedisError::InvalidInput(msg));
                }
                Ok(score as u64)
            })
            .collect()
    }

    /// Advances a user's event cursor for `hs_id`, rejecting a value that would
    /// move it backward. Mirrors `Homeserver::put_to_index` so both cursor types
    /// share one monotonicity rule.
    ///
    /// The compare and the write run as one script ([`WRITE_SCRIPT`]), so writers
    /// racing on the same `(user_id, hs_id)` cannot rewind it: whichever lands
    /// second, the lower value is rejected.
    pub async fn write(user_id: &str, hs_id: &str, cursor: u64) -> RedisResult<()> {
        let key = format!("{SORTED_PREFIX}:{}", user_hs_cursor_key(user_id).join(":"));
        let mut redis_conn = get_redis_conn().await?;
        let written: i64 = WRITE_SCRIPT
            .key(key)
            .arg(cursor)
            .arg(hs_id)
            .invoke_async(&mut redis_conn)
            .await?;

        match written {
            1 => Ok(()),
            _ => Err(cursor_backwards_error()),
        }
    }

    /// Seeds the user's cursor to `0` the first time we track them on `hs_id`,
    /// and no-ops if one already exists (e.g. re-ingestion after it advanced),
    /// establishing the floor without ever rewinding it.
    ///
    /// Seeded atomically (`ZADD NX`), and silently: the caller is the user
    /// ingestor, running on the primary-HS and retry-processor tasks, and it
    /// publishes the user's `HOSTED_BY` edge before seeding — so the concurrent
    /// external-HS task can fetch, index and `write` a real cursor in between. A
    /// read-then-write would clobber that with 0 and re-index the user's history.
    pub async fn init(user_id: &str, hs_id: &str) -> RedisResult<()> {
        let key = user_hs_cursor_key(user_id);
        Self::add_index_sorted_set_if_absent(&key, 0.0, hs_id, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{types::DynError, utils::test_utils::random_pubky_id, StackConfig, StackManager};

    /// `write` persists a per-user cursor that `read` reads back, missing entries
    /// default to 0, and a lower re-write is rejected rather than rewinding.
    #[tokio_shared_rt::test(shared)]
    async fn test_hs_cursor_read_write_roundtrip() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        // Random IDs keep the test isolated from mock data and other tests.
        let user_with_cursor = random_pubky_id().to_string();
        let user_without_cursor = random_pubky_id().to_string();
        let hs_id = random_pubky_id().to_string();

        // A user with no stored cursor reads back as 0.
        let cursors = UserHsCursor::read(&[user_with_cursor.as_str()], &hs_id).await?;
        assert_eq!(cursors, vec![0]);

        // A written cursor round-trips; a user without an entry stays at 0.
        UserHsCursor::write(&user_with_cursor, &hs_id, 42).await?;
        let cursors = UserHsCursor::read(
            &[user_with_cursor.as_str(), user_without_cursor.as_str()],
            &hs_id,
        )
        .await?;
        assert_eq!(cursors, vec![42, 0]);

        // Writing a higher cursor advances the value.
        UserHsCursor::write(&user_with_cursor, &hs_id, 100).await?;
        let cursors = UserHsCursor::read(&[user_with_cursor.as_str()], &hs_id).await?;
        assert_eq!(cursors, vec![100]);

        // Re-writing the same cursor is accepted.
        UserHsCursor::write(&user_with_cursor, &hs_id, 100).await?;

        // Writing a lower cursor is rejected and leaves the existing value intact.
        let err = UserHsCursor::write(&user_with_cursor, &hs_id, 50)
            .await
            .expect_err("backward cursor must be rejected");
        assert!(matches!(err, RedisError::InvalidInput(_)));
        let cursors = UserHsCursor::read(&[user_with_cursor.as_str()], &hs_id).await?;
        assert_eq!(cursors, vec![100]);

        Ok(())
    }

    /// Two writers racing on one cursor never rewind it: whichever lands second,
    /// the lower value is rejected. A read-then-write lets both read the old value
    /// and the last write win.
    #[tokio_shared_rt::test(shared)]
    async fn test_hs_cursor_racing_writes_never_rewind() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        for _ in 0..20 {
            let user_id = random_pubky_id().to_string();
            let hs_id = random_pubky_id().to_string();

            let (high, low) = tokio::join!(
                UserHsCursor::write(&user_id, &hs_id, 10),
                UserHsCursor::write(&user_id, &hs_id, 5),
            );

            high?;
            if let Err(err) = low {
                assert!(matches!(err, RedisError::InvalidInput(_)), "{err}");
            }
            let cursors = UserHsCursor::read(&[user_id.as_str()], &hs_id).await?;
            assert_eq!(cursors, vec![10]);
        }

        Ok(())
    }

    /// `init` seeds a fresh user's cursor to 0 and never rewinds an existing one.
    #[tokio_shared_rt::test(shared)]
    async fn test_hs_cursor_init_seeds_without_rewinding() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let user_id = random_pubky_id().to_string();
        let hs_id = random_pubky_id().to_string();

        // First init seeds the starting cursor at 0.
        UserHsCursor::init(&user_id, &hs_id).await?;
        let cursors = UserHsCursor::read(&[user_id.as_str()], &hs_id).await?;
        assert_eq!(cursors, vec![0]);

        // Advance the cursor, then init again (e.g. re-ingestion): it must not rewind.
        UserHsCursor::write(&user_id, &hs_id, 100).await?;
        UserHsCursor::init(&user_id, &hs_id).await?;
        let cursors = UserHsCursor::read(&[user_id.as_str()], &hs_id).await?;
        assert_eq!(cursors, vec![100]);

        Ok(())
    }

    /// `read` rejects a stored negative cursor score instead of wrapping it into a huge `u64`.
    #[tokio_shared_rt::test(shared)]
    async fn test_hs_cursor_read_rejects_negative_score() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        let user_id = random_pubky_id().to_string();
        let hs_id = random_pubky_id().to_string();

        // `write` only accepts u64, so store a negative score directly.
        let key = user_hs_cursor_key(&user_id);
        UserHsCursor::put_index_sorted_set(&key, &[(-1.0, hs_id.as_str())], None, None).await?;

        let err = UserHsCursor::read(&[user_id.as_str()], &hs_id)
            .await
            .expect_err("negative cursor score should error");
        assert!(matches!(err, RedisError::InvalidInput(_)));

        Ok(())
    }
}
