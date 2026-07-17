use serde::{Deserialize, Serialize};

use crate::db::kv::{RedisError, RedisResult};
use crate::db::RedisOps;

const USER_HS_CURSOR: [&str; 2] = ["Users", "Homeservers"];

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

    /// Persists a single user's event cursor for `hs_id` without moving it backward.
    pub async fn write(user_id: &str, hs_id: &str, cursor: u64) -> RedisResult<()> {
        let stored_cursor = Self::read(&[user_id], hs_id)
            .await?
            .into_iter()
            .next()
            .unwrap_or_default();
        let cursor = cursor.max(stored_cursor);
        let key = user_hs_cursor_key(user_id);
        Self::put_index_sorted_set(&key, &[(cursor as f64, hs_id)], None, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{types::DynError, utils::test_utils::random_pubky_id, StackConfig, StackManager};

    /// `write` persists a per-user cursor that `read` reads back, missing entries
    /// default to 0, and lower re-writes do not rewind the value.
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

        // Writing a lower cursor leaves the existing value intact.
        UserHsCursor::write(&user_with_cursor, &hs_id, 50).await?;
        let cursors = UserHsCursor::read(&[user_with_cursor.as_str()], &hs_id).await?;
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
