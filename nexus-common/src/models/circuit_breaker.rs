use crate::db::kv::RedisResult;
use crate::db::RedisOps;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Circuit breaker states for homeserver health tracking.
///
/// Transitions:
/// - **Closed → Open**: when consecutive failures reach `FAILURE_THRESHOLD`
/// - **Open → HalfOpen**: when `cooldown_secs` has elapsed since `last_failure_ts`
/// - **HalfOpen → Closed**: on success (probe passed)
/// - **HalfOpen → Open**: on failure (probe failed, cooldown restarts)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Persisted circuit breaker health record for a homeserver.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HomeserverCircuitBreaker {
    pub state: CircuitState,
    pub fail_count: u32,
    /// Unix timestamp (seconds) of the last failure
    pub last_failure_ts: u64,
}

impl Default for HomeserverCircuitBreaker {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            fail_count: 0,
            last_failure_ts: 0,
        }
    }
}

impl RedisOps for HomeserverCircuitBreaker {}

/// Number of consecutive failures before opening the circuit.
const FAILURE_THRESHOLD: u32 = 5;

/// Seconds to wait before transitioning Open → HalfOpen.
const DEFAULT_COOLDOWN_SECS: u64 = 300; // 5 minutes

impl HomeserverCircuitBreaker {
    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Loads the circuit breaker for a homeserver from Redis.
    /// Returns `None` if no record exists (treated as Closed).
    pub async fn get(hs_id: &str) -> RedisResult<Option<Self>> {
        Self::try_from_index_json(&[hs_id], None).await
    }

    /// Persists the circuit breaker state to Redis.
    async fn save(&self, hs_id: &str) -> RedisResult<()> {
        self.put_index_json(&[hs_id], None, None).await
    }

    /// Removes the circuit breaker record from Redis (resets to default Closed).
    pub async fn delete(hs_id: &str) -> RedisResult<()> {
        Self::remove_from_index_multiple_json(&[&[hs_id]]).await
    }

    /// Returns the effective state of a homeserver's circuit, applying time-based transitions.
    ///
    /// - If no record exists → Closed
    /// - If Open and cooldown has elapsed → transitions to HalfOpen (and persists)
    /// - Otherwise returns stored state
    pub async fn effective_state(hs_id: &str) -> RedisResult<CircuitState> {
        Self::effective_state_with_cooldown(hs_id, DEFAULT_COOLDOWN_SECS).await
    }

    /// Like [`effective_state`] but with a configurable cooldown.
    pub async fn effective_state_with_cooldown(
        hs_id: &str,
        cooldown_secs: u64,
    ) -> RedisResult<CircuitState> {
        let Some(mut cb) = Self::get(hs_id).await? else {
            return Ok(CircuitState::Closed);
        };

        if cb.state == CircuitState::Open {
            let elapsed = Self::now_secs().saturating_sub(cb.last_failure_ts);
            if elapsed >= cooldown_secs {
                debug!("Circuit breaker for {hs_id}: Open → HalfOpen (cooldown elapsed)");
                cb.state = CircuitState::HalfOpen;
                cb.save(hs_id).await?;
            }
        }

        Ok(cb.state)
    }

    /// Records a successful run.
    ///
    /// - **No record (Closed)**: no-op
    /// - **Closed with partial failures**: resets (deletes record)
    /// - **HalfOpen**: probe succeeded → resets to Closed (deletes record)
    /// - **Open**: ignored — a stale/delayed success should not bypass the cooldown
    pub async fn record_success(hs_id: &str) -> RedisResult<()> {
        let Some(cb) = Self::get(hs_id).await? else {
            return Ok(()); // Already closed (no record)
        };

        match cb.state {
            CircuitState::Open => {
                debug!(
                    "Circuit breaker for {hs_id}: ignoring success while Open (fail_count={})",
                    cb.fail_count
                );
                Ok(())
            }
            CircuitState::HalfOpen => {
                info!("Circuit breaker for {hs_id}: HalfOpen → Closed (probe succeeded)");
                Self::delete(hs_id).await
            }
            CircuitState::Closed => {
                debug!("Circuit breaker for {hs_id}: resetting fail count on success");
                Self::delete(hs_id).await
            }
        }
    }

    /// Records a failure. Increments `fail_count` and may open the circuit.
    pub async fn record_failure(hs_id: &str) -> RedisResult<()> {
        let mut cb = Self::get(hs_id).await?.unwrap_or_default();

        cb.fail_count = cb.fail_count.saturating_add(1);
        cb.last_failure_ts = Self::now_secs();

        match cb.state {
            CircuitState::Closed if cb.fail_count >= FAILURE_THRESHOLD => {
                warn!(
                    "Circuit breaker for {hs_id}: Closed → Open (failed {} times)",
                    cb.fail_count
                );
                cb.state = CircuitState::Open;
            }
            CircuitState::HalfOpen => {
                warn!("Circuit breaker for {hs_id}: HalfOpen → Open (probe failed)");
                cb.state = CircuitState::Open;
            }
            _ => {
                debug!(
                    "Circuit breaker for {hs_id}: failure #{} (state={:?})",
                    cb.fail_count, cb.state
                );
            }
        }

        cb.save(hs_id).await
    }

    /// Returns true if the homeserver should be skipped (circuit is Open).
    ///
    /// HalfOpen homeservers are NOT skipped — they get exactly one probe attempt.
    pub async fn should_skip(hs_id: &str) -> RedisResult<bool> {
        let state = Self::effective_state(hs_id).await?;
        Ok(state == CircuitState::Open)
    }

    /// Filters a list of homeserver IDs, removing those whose circuit is Open.
    ///
    /// HalfOpen homeservers are kept (they need a probe attempt).
    pub async fn filter_available(hs_ids: Vec<String>) -> RedisResult<Vec<String>> {
        let mut available = Vec::with_capacity(hs_ids.len());
        for hs_id in hs_ids {
            match Self::should_skip(&hs_id).await {
                Ok(true) => {
                    debug!("Skipping homeserver {hs_id}: circuit is Open");
                }
                Ok(false) => {
                    available.push(hs_id);
                }
                Err(e) => {
                    // On Redis errors, don't skip — fail open
                    warn!("Circuit breaker check failed for {hs_id}, allowing through: {e}");
                    available.push(hs_id);
                }
            }
        }
        Ok(available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{types::DynError, StackConfig, StackManager};

    #[tokio_shared_rt::test(shared)]
    async fn test_default_state_is_closed() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_default_state_hs";
        // Clean up any previous state
        HomeserverCircuitBreaker::delete(hs_id).await?;

        let state = HomeserverCircuitBreaker::effective_state(hs_id).await?;
        assert_eq!(state, CircuitState::Closed);
        assert!(!HomeserverCircuitBreaker::should_skip(hs_id).await?);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_failures_open_circuit() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_failures_open_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Record failures below threshold
        for _ in 0..FAILURE_THRESHOLD - 1 {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
            assert!(!HomeserverCircuitBreaker::should_skip(hs_id).await?);
        }

        // One more failure should open the circuit
        HomeserverCircuitBreaker::record_failure(hs_id).await?;
        assert!(HomeserverCircuitBreaker::should_skip(hs_id).await?);

        let state = HomeserverCircuitBreaker::effective_state(hs_id).await?;
        assert_eq!(state, CircuitState::Open);

        // Clean up
        HomeserverCircuitBreaker::delete(hs_id).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_success_resets_circuit() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_success_resets_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Open the circuit
        for _ in 0..FAILURE_THRESHOLD {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
        }
        assert!(HomeserverCircuitBreaker::should_skip(hs_id).await?);

        // Simulate cooldown elapsed by directly setting HalfOpen state
        let mut cb = HomeserverCircuitBreaker::get(hs_id).await?.unwrap();
        cb.state = CircuitState::HalfOpen;
        cb.save(hs_id).await?;

        // Should not be skipped (HalfOpen allows probe)
        assert!(!HomeserverCircuitBreaker::should_skip(hs_id).await?);

        // Success should close the circuit
        HomeserverCircuitBreaker::record_success(hs_id).await?;
        let state = HomeserverCircuitBreaker::effective_state(hs_id).await?;
        assert_eq!(state, CircuitState::Closed);
        assert!(!HomeserverCircuitBreaker::should_skip(hs_id).await?);

        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_half_open_failure_reopens_circuit() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_half_open_fail_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Open the circuit
        for _ in 0..FAILURE_THRESHOLD {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
        }

        // Manually transition to HalfOpen
        let mut cb = HomeserverCircuitBreaker::get(hs_id).await?.unwrap();
        cb.state = CircuitState::HalfOpen;
        cb.save(hs_id).await?;

        // Failure in HalfOpen should reopen
        HomeserverCircuitBreaker::record_failure(hs_id).await?;
        let state = HomeserverCircuitBreaker::effective_state(hs_id).await?;
        assert_eq!(state, CircuitState::Open);
        assert!(HomeserverCircuitBreaker::should_skip(hs_id).await?);

        // Clean up
        HomeserverCircuitBreaker::delete(hs_id).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_cooldown_transitions_to_half_open() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_cooldown_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Open the circuit with old timestamp
        let cb = HomeserverCircuitBreaker {
            state: CircuitState::Open,
            fail_count: FAILURE_THRESHOLD,
            last_failure_ts: HomeserverCircuitBreaker::now_secs() - DEFAULT_COOLDOWN_SECS - 1,
        };
        cb.save(hs_id).await?;

        // effective_state should see cooldown elapsed and transition to HalfOpen
        let state = HomeserverCircuitBreaker::effective_state(hs_id).await?;
        assert_eq!(state, CircuitState::HalfOpen);
        assert!(!HomeserverCircuitBreaker::should_skip(hs_id).await?);

        // Clean up
        HomeserverCircuitBreaker::delete(hs_id).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_filter_available() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let open_hs = "test_filter_open_hs";
        let closed_hs = "test_filter_closed_hs";

        // Clean up
        HomeserverCircuitBreaker::delete(open_hs).await?;
        HomeserverCircuitBreaker::delete(closed_hs).await?;

        // Open the circuit for one HS
        for _ in 0..FAILURE_THRESHOLD {
            HomeserverCircuitBreaker::record_failure(open_hs).await?;
        }

        let input = vec![open_hs.to_string(), closed_hs.to_string()];
        let result = HomeserverCircuitBreaker::filter_available(input).await?;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], closed_hs);

        // Clean up
        HomeserverCircuitBreaker::delete(open_hs).await?;
        Ok(())
    }

    /// Verifies that a success while Closed (no record) is a no-op and doesn't create a record.
    #[tokio_shared_rt::test(shared)]
    async fn test_success_on_clean_state_is_noop() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_success_noop_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Success on a homeserver with no record should be fine
        HomeserverCircuitBreaker::record_success(hs_id).await?;

        // No record should have been created
        let cb = HomeserverCircuitBreaker::get(hs_id).await?;
        assert!(cb.is_none());

        Ok(())
    }

    /// Verifies that a success while Open is ignored — the circuit stays Open.
    #[tokio_shared_rt::test(shared)]
    async fn test_success_while_open_is_ignored() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_success_open_ignored_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Open the circuit
        for _ in 0..FAILURE_THRESHOLD {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
        }
        assert!(HomeserverCircuitBreaker::should_skip(hs_id).await?);

        // A success while Open should be ignored (stale/delayed result)
        HomeserverCircuitBreaker::record_success(hs_id).await?;

        // Circuit should still be Open
        let cb = HomeserverCircuitBreaker::get(hs_id).await?.unwrap();
        assert_eq!(cb.state, CircuitState::Open);
        assert_eq!(cb.fail_count, FAILURE_THRESHOLD);
        assert!(HomeserverCircuitBreaker::should_skip(hs_id).await?);

        // Clean up
        HomeserverCircuitBreaker::delete(hs_id).await?;
        Ok(())
    }

    /// Verifies that a success mid-way through failures (before threshold) resets the counter.
    #[tokio_shared_rt::test(shared)]
    async fn test_success_resets_partial_failure_count() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_partial_reset_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Accumulate some failures (but below threshold)
        for _ in 0..FAILURE_THRESHOLD - 1 {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
        }
        let cb = HomeserverCircuitBreaker::get(hs_id).await?.unwrap();
        assert_eq!(cb.fail_count, FAILURE_THRESHOLD - 1);
        assert_eq!(cb.state, CircuitState::Closed);

        // A success should wipe the record
        HomeserverCircuitBreaker::record_success(hs_id).await?;
        let cb = HomeserverCircuitBreaker::get(hs_id).await?;
        assert!(cb.is_none());

        // Subsequent failures start from zero — need full threshold again to open
        for _ in 0..FAILURE_THRESHOLD - 1 {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
        }
        assert!(!HomeserverCircuitBreaker::should_skip(hs_id).await?);

        // Clean up
        HomeserverCircuitBreaker::delete(hs_id).await?;
        Ok(())
    }

    /// Verifies that an Open circuit whose cooldown has NOT elapsed stays Open.
    #[tokio_shared_rt::test(shared)]
    async fn test_open_stays_open_before_cooldown() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_open_no_cooldown_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Open the circuit with a recent timestamp
        let cb = HomeserverCircuitBreaker {
            state: CircuitState::Open,
            fail_count: FAILURE_THRESHOLD,
            last_failure_ts: HomeserverCircuitBreaker::now_secs(),
        };
        cb.save(hs_id).await?;

        // Should still be Open (cooldown not elapsed)
        let state = HomeserverCircuitBreaker::effective_state(hs_id).await?;
        assert_eq!(state, CircuitState::Open);
        assert!(HomeserverCircuitBreaker::should_skip(hs_id).await?);

        // Clean up
        HomeserverCircuitBreaker::delete(hs_id).await?;
        Ok(())
    }

    /// Verifies the full lifecycle: Closed → Open → HalfOpen → Closed.
    #[tokio_shared_rt::test(shared)]
    async fn test_full_lifecycle() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_full_lifecycle_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // 1. Start Closed
        assert_eq!(
            HomeserverCircuitBreaker::effective_state(hs_id).await?,
            CircuitState::Closed
        );

        // 2. Accumulate failures → Open
        for _ in 0..FAILURE_THRESHOLD {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
        }
        assert_eq!(
            HomeserverCircuitBreaker::effective_state(hs_id).await?,
            CircuitState::Open
        );

        // 3. Simulate cooldown → HalfOpen
        let mut cb = HomeserverCircuitBreaker::get(hs_id).await?.unwrap();
        cb.last_failure_ts = HomeserverCircuitBreaker::now_secs() - DEFAULT_COOLDOWN_SECS - 1;
        cb.save(hs_id).await?;
        assert_eq!(
            HomeserverCircuitBreaker::effective_state(hs_id).await?,
            CircuitState::HalfOpen
        );

        // 4. Probe succeeds → Closed
        HomeserverCircuitBreaker::record_success(hs_id).await?;
        assert_eq!(
            HomeserverCircuitBreaker::effective_state(hs_id).await?,
            CircuitState::Closed
        );
        assert!(HomeserverCircuitBreaker::get(hs_id).await?.is_none());

        Ok(())
    }

    /// Verifies the rejection lifecycle: Closed → Open → HalfOpen → Open (probe fails).
    #[tokio_shared_rt::test(shared)]
    async fn test_failed_probe_lifecycle() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_failed_probe_lifecycle_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Closed → Open
        for _ in 0..FAILURE_THRESHOLD {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
        }

        // Open → HalfOpen (simulate cooldown)
        let mut cb = HomeserverCircuitBreaker::get(hs_id).await?.unwrap();
        cb.last_failure_ts = HomeserverCircuitBreaker::now_secs() - DEFAULT_COOLDOWN_SECS - 1;
        cb.save(hs_id).await?;
        assert_eq!(
            HomeserverCircuitBreaker::effective_state(hs_id).await?,
            CircuitState::HalfOpen
        );

        // HalfOpen → Open (probe fails)
        HomeserverCircuitBreaker::record_failure(hs_id).await?;
        assert_eq!(
            HomeserverCircuitBreaker::effective_state(hs_id).await?,
            CircuitState::Open
        );
        assert!(HomeserverCircuitBreaker::should_skip(hs_id).await?);

        // Clean up
        HomeserverCircuitBreaker::delete(hs_id).await?;
        Ok(())
    }

    /// Verifies that additional failures beyond the threshold keep the circuit Open
    /// and increment the counter.
    #[tokio_shared_rt::test(shared)]
    async fn test_extra_failures_beyond_threshold() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_extra_failures_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Open the circuit
        for _ in 0..FAILURE_THRESHOLD {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
        }

        // Record more failures while Open
        for _ in 0..3 {
            HomeserverCircuitBreaker::record_failure(hs_id).await?;
        }

        let cb = HomeserverCircuitBreaker::get(hs_id).await?.unwrap();
        assert_eq!(cb.state, CircuitState::Open);
        assert_eq!(cb.fail_count, FAILURE_THRESHOLD + 3);

        // Clean up
        HomeserverCircuitBreaker::delete(hs_id).await?;
        Ok(())
    }

    /// Verifies that filter_available keeps HalfOpen homeservers (they need a probe).
    #[tokio_shared_rt::test(shared)]
    async fn test_filter_available_keeps_half_open() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let half_open_hs = "test_filter_halfopen_hs";
        let open_hs = "test_filter_open2_hs";
        let closed_hs = "test_filter_closed2_hs";

        HomeserverCircuitBreaker::delete(half_open_hs).await?;
        HomeserverCircuitBreaker::delete(open_hs).await?;
        HomeserverCircuitBreaker::delete(closed_hs).await?;

        // Set one to HalfOpen
        let cb = HomeserverCircuitBreaker {
            state: CircuitState::HalfOpen,
            fail_count: FAILURE_THRESHOLD,
            last_failure_ts: HomeserverCircuitBreaker::now_secs() - DEFAULT_COOLDOWN_SECS - 1,
        };
        cb.save(half_open_hs).await?;

        // Set one to Open (recent failure, no cooldown yet)
        let cb = HomeserverCircuitBreaker {
            state: CircuitState::Open,
            fail_count: FAILURE_THRESHOLD,
            last_failure_ts: HomeserverCircuitBreaker::now_secs(),
        };
        cb.save(open_hs).await?;

        // closed_hs has no record (Closed)

        let input = vec![
            half_open_hs.to_string(),
            open_hs.to_string(),
            closed_hs.to_string(),
        ];
        let result = HomeserverCircuitBreaker::filter_available(input).await?;

        assert_eq!(result.len(), 2);
        assert!(result.contains(&half_open_hs.to_string()));
        assert!(result.contains(&closed_hs.to_string()));
        assert!(!result.contains(&open_hs.to_string()));

        // Clean up
        HomeserverCircuitBreaker::delete(half_open_hs).await?;
        HomeserverCircuitBreaker::delete(open_hs).await?;
        Ok(())
    }

    /// Verifies that filter_available with an empty list returns an empty list.
    #[tokio_shared_rt::test(shared)]
    async fn test_filter_available_empty_input() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let result = HomeserverCircuitBreaker::filter_available(vec![]).await?;
        assert!(result.is_empty());

        Ok(())
    }

    /// Verifies that delete is idempotent — deleting a non-existent record is fine.
    #[tokio_shared_rt::test(shared)]
    async fn test_delete_nonexistent_is_ok() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_delete_nonexistent_hs";
        // Delete twice — both should succeed
        HomeserverCircuitBreaker::delete(hs_id).await?;
        HomeserverCircuitBreaker::delete(hs_id).await?;

        Ok(())
    }

    /// Verifies that the configurable cooldown in effective_state_with_cooldown works.
    #[tokio_shared_rt::test(shared)]
    async fn test_custom_cooldown() -> Result<(), DynError> {
        StackManager::setup("unit-circuit-breaker-test", &StackConfig::default()).await?;

        let hs_id = "test_custom_cooldown_hs";
        HomeserverCircuitBreaker::delete(hs_id).await?;

        // Open with a failure 10 seconds ago
        let cb = HomeserverCircuitBreaker {
            state: CircuitState::Open,
            fail_count: FAILURE_THRESHOLD,
            last_failure_ts: HomeserverCircuitBreaker::now_secs() - 10,
        };
        cb.save(hs_id).await?;

        // With a 60s cooldown, it should still be Open
        let state = HomeserverCircuitBreaker::effective_state_with_cooldown(hs_id, 60).await?;
        assert_eq!(state, CircuitState::Open);

        // With a 5s cooldown, it should transition to HalfOpen
        let state = HomeserverCircuitBreaker::effective_state_with_cooldown(hs_id, 5).await?;
        assert_eq!(state, CircuitState::HalfOpen);

        // Clean up
        HomeserverCircuitBreaker::delete(hs_id).await?;
        Ok(())
    }
}
