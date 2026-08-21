use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::processors::MediaProcessorError;

/// How many requests may queue per permit before we shed instead of parking them.
const MAX_WAITING_PER_PERMIT: usize = 4;

/// Default wait for a permit before shedding.
const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

/// The permits every media subprocess must hold. Gates are handles on a pool: clone it
/// to hand another gate the *same* permits — building a second pool instead would let
/// both gates run `permits` subprocesses each.
#[derive(Clone)]
pub struct MediaPermits {
    semaphore: Arc<Semaphore>,
    /// The pool's size. Kept separately because `available_permits` is the live free
    /// count, which is not what a gate sizing itself against the pool wants.
    capacity: usize,
}

impl MediaPermits {
    /// A pool allowing at most `permits` concurrent subprocesses.
    pub fn new(permits: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(permits)),
            capacity: permits,
        }
    }

    /// Take a permit only if one is free right now.
    fn try_acquire(&self) -> Result<OwnedSemaphorePermit, MediaProcessorError> {
        Arc::clone(&self.semaphore)
            .try_acquire_owned()
            .map_err(|_| MediaProcessorError::AtCapacity)
    }

    /// Wait for a permit. Callers are responsible for bounding the wait.
    async fn acquire(&self) -> Result<OwnedSemaphorePermit, MediaProcessorError> {
        Arc::clone(&self.semaphore)
            .acquire_owned()
            .await
            .map_err(|_closed| MediaProcessorError::AtCapacity)
    }
}

/// Bounded concurrency gate for media subprocesses (ImageMagick/ffmpeg). Implementors
/// differ only in what they do when no permit is free.
#[async_trait]
pub trait MediaGate: Send + Sync {
    /// Acquire a permit or shed with `AtCapacity`. The permit is owned so it can be
    /// moved into the task running the subprocess and outlive the caller's request.
    async fn acquire(&self) -> Result<OwnedSemaphorePermit, MediaProcessorError>;
}

/// Queues for a permit, shedding once the queue is full or the wait times out.
/// The default for callers with nothing cheaper to serve.
pub struct QueuedGate {
    permits: MediaPermits,
    /// Requests currently parked waiting for a permit.
    waiting: AtomicUsize,
    /// Cap on parked requests. The semaphore's own wait list is unbounded, so without
    /// this a burst parks arbitrarily many requests, each holding a connection and task
    /// for the full acquire timeout — moving exhaustion from subprocesses to the queue.
    max_waiting: usize,
    /// How long a queued caller waits for a permit before shedding. With the queue
    /// bounded this is a backstop, not the primary shed mechanism.
    acquire_timeout: Duration,
}

impl QueuedGate {
    /// A gate over `permits`, queueing up to `MAX_WAITING_PER_PERMIT` callers per permit.
    pub fn new(permits: MediaPermits) -> Self {
        let max_waiting = permits.capacity.max(1) * MAX_WAITING_PER_PERMIT;
        Self::with_limits(permits, max_waiting, DEFAULT_ACQUIRE_TIMEOUT)
    }

    /// Full control over the shed thresholds, for tests and tuning.
    pub fn with_limits(
        permits: MediaPermits,
        max_waiting: usize,
        acquire_timeout: Duration,
    ) -> Self {
        Self {
            permits,
            waiting: AtomicUsize::new(0),
            max_waiting,
            acquire_timeout,
        }
    }
}

#[async_trait]
impl MediaGate for QueuedGate {
    async fn acquire(&self) -> Result<OwnedSemaphorePermit, MediaProcessorError> {
        // Fast path: capacity is free, so never touch the queue or the timer.
        if let Ok(permit) = self.permits.try_acquire() {
            return Ok(permit);
        }

        // A pool that holds no permits can never grant one, and it cannot grow, so
        // parking here would burn the whole timeout to reach the answer we have now.
        if self.permits.capacity == 0 {
            return Err(MediaProcessorError::AtCapacity);
        }

        // Queue is already deep enough; shed now rather than park another request.
        let _waiting = WaitingGuard::enter(&self.waiting, self.max_waiting)?;

        match tokio::time::timeout(self.acquire_timeout, self.permits.acquire()).await {
            Ok(result) => result,
            Err(_elapsed) => Err(MediaProcessorError::AtCapacity), // waited too long -> shed load
        }
    }
}

/// Sheds the moment no permit is free, for callers whose fallback is good enough that
/// the chance of winning a permit before the timeout is not worth the latency (avatars).
pub struct FailFastGate {
    permits: MediaPermits,
}

impl FailFastGate {
    /// A gate over `permits`. Clone the pool from the queued gate's to share its limit.
    pub fn new(permits: MediaPermits) -> Self {
        Self { permits }
    }
}

#[async_trait]
impl MediaGate for FailFastGate {
    async fn acquire(&self) -> Result<OwnedSemaphorePermit, MediaProcessorError> {
        // No queue and no timer to reach: a burst here cannot push queued callers over
        // the cap, because this gate has no access to their counter at all.
        self.permits.try_acquire()
    }
}

/// Counts a parked request, releasing on drop so cancelled requests don't leak a slot.
/// This has to be a guard rather than a pair of counter updates in `acquire`: a cancelled
/// request drops its future at the await, so a decrement written after it never runs.
struct WaitingGuard<'a>(&'a AtomicUsize);

impl<'a> WaitingGuard<'a> {
    fn enter(waiting: &'a AtomicUsize, max: usize) -> Result<Self, MediaProcessorError> {
        if waiting.fetch_add(1, Ordering::Relaxed) >= max {
            waiting.fetch_sub(1, Ordering::Relaxed);
            return Err(MediaProcessorError::AtCapacity);
        }
        Ok(Self(waiting))
    }
}

impl Drop for WaitingGuard<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use tokio::time::Duration;

    use super::{
        FailFastGate, MediaGate, MediaPermits, QueuedGate, DEFAULT_ACQUIRE_TIMEOUT,
        MAX_WAITING_PER_PERMIT,
    };

    #[tokio_shared_rt::test(shared)]
    async fn test_contended_acquire_sheds_then_succeeds_once_released() {
        let gate = QueuedGate::with_limits(MediaPermits::new(1), 4, Duration::from_millis(50));

        let permit = gate.acquire().await.expect("first acquire should succeed");

        // While the only permit is held, a contended acquire sheds after the timeout.
        assert!(
            gate.acquire().await.is_err(),
            "acquire must shed while the only permit is held"
        );

        drop(permit);

        assert!(
            gate.acquire().await.is_ok(),
            "acquire must succeed once the permit is released"
        );
    }

    // A burst must not park unboundedly: once the queue cap is reached, further
    // callers shed immediately instead of waiting out the acquire timeout.
    #[tokio_shared_rt::test(shared)]
    async fn test_queue_cap_sheds_without_parking() {
        let gate = Arc::new(QueuedGate::with_limits(
            MediaPermits::new(1),
            2,
            Duration::from_secs(30),
        ));

        let _permit = gate.acquire().await.expect("first acquire should succeed");

        // Fill the queue with the two waiters it allows.
        let mut parked = Vec::new();
        for _ in 0..2 {
            let gate = Arc::clone(&gate);
            parked.push(tokio::spawn(async move { gate.acquire().await.is_ok() }));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;

        // The next caller is over the cap, so it must shed rather than park for 30s.
        let start = tokio::time::Instant::now();
        assert!(
            gate.acquire().await.is_err(),
            "acquire beyond the queue cap must shed"
        );
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "over-cap acquire must shed immediately, waited {:?}",
            start.elapsed()
        );

        for handle in parked {
            handle.abort();
            let _ = handle.await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Cancelled callers must give their queue slots back, or the cap would ratchet
        // down until every queued caller sheds.
        assert_eq!(
            gate.waiting.load(Ordering::Relaxed),
            0,
            "cancelled callers must release their queue slots"
        );
    }

    // A fail-fast gate must shed the moment no permit is free rather than parking,
    // and must draw on the same permits as the gate it came from.
    #[tokio_shared_rt::test(shared)]
    async fn test_fail_fast_gate_sheds_without_queueing() {
        let permits = MediaPermits::new(1);
        let gate = QueuedGate::with_limits(permits.clone(), 4, Duration::from_secs(30));
        let fast = FailFastGate::new(permits);

        let permit = gate.acquire().await.expect("first acquire should succeed");

        let start = tokio::time::Instant::now();
        assert!(
            fast.acquire().await.is_err(),
            "fail-fast acquire must shed while the only permit is held"
        );
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "fail-fast acquire must not queue, waited {:?}",
            start.elapsed()
        );

        drop(permit);

        assert!(
            fast.acquire().await.is_ok(),
            "fail-fast acquire must succeed once the shared permit is released"
        );
    }

    // Fail-fast callers must leave the queue counter alone, or a burst of them would
    // shed unrelated queued traffic. `FailFastGate::acquire` can't touch it by
    // construction, so this guards against a future edit routing it through the queue.
    #[tokio_shared_rt::test(shared)]
    async fn test_fail_fast_does_not_consume_queue_slots() {
        let permits = MediaPermits::new(1);
        let gate = Arc::new(QueuedGate::with_limits(
            permits.clone(),
            1,
            DEFAULT_ACQUIRE_TIMEOUT,
        ));
        let fast = FailFastGate::new(permits);

        let permit = gate.acquire().await.expect("first acquire should succeed");

        // Hammer the fail-fast gate while the only permit is held.
        for _ in 0..50 {
            assert!(fast.acquire().await.is_err(), "no permit is free");
        }
        assert_eq!(
            gate.waiting.load(Ordering::Relaxed),
            0,
            "fail-fast acquires must not register as queued callers"
        );

        // The single queue slot must still be available to a queued caller, which
        // parks (and is served once the permit drops) instead of shedding.
        let queued = tokio::spawn({
            let gate = Arc::clone(&gate);
            async move { gate.acquire().await.is_ok() }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(permit);

        assert!(
            queued.await.expect("queued caller should not panic"),
            "a queued caller must still find a free queue slot after a fail-fast burst"
        );
    }

    // A gate over an empty pool sheds at once: no permit can ever appear, so waiting out
    // the acquire timeout only delays the same answer.
    #[tokio_shared_rt::test(shared)]
    async fn test_empty_pool_sheds_without_waiting() {
        let gate = QueuedGate::new(MediaPermits::new(0));

        let start = tokio::time::Instant::now();
        assert!(
            gate.acquire().await.is_err(),
            "a pool with no permits must shed"
        );
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "shedding must not wait out the acquire timeout, waited {:?}",
            start.elapsed()
        );
    }

    // A gate sizes its queue against the pool's capacity, not the permits free when it
    // happens to be built — otherwise a gate added to a pool that is already busy would
    // silently shed traffic it was configured to queue.
    #[tokio_shared_rt::test(shared)]
    async fn test_queue_cap_follows_capacity_not_free_permits() {
        let permits = MediaPermits::new(4);
        let held: Vec<_> = (0..4)
            .map(|_| permits.try_acquire().expect("pool starts empty"))
            .collect();

        let gate = QueuedGate::new(permits.clone());

        assert_eq!(
            gate.max_waiting,
            4 * MAX_WAITING_PER_PERMIT,
            "queue cap must come from the pool's capacity, not its free permits"
        );
        drop(held);
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_peak_concurrency_bounded() {
        // Uncapped queue: this test is about the permit count, not the shed threshold.
        let gate = Arc::new(QueuedGate::with_limits(
            MediaPermits::new(2),
            usize::MAX,
            DEFAULT_ACQUIRE_TIMEOUT,
        ));
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        for _ in 0..10 {
            let gate = Arc::clone(&gate);
            let concurrent = Arc::clone(&concurrent);
            let peak = Arc::clone(&peak);
            let handle = tokio::spawn(async move {
                let _permit = gate.acquire().await.unwrap();
                // Record peak concurrency.
                let cur = concurrent.fetch_add(1, Ordering::Relaxed) + 1;
                peak.fetch_max(cur, Ordering::Relaxed);

                // Simulate work.
                tokio::time::sleep(Duration::from_millis(50)).await;

                concurrent.fetch_sub(1, Ordering::Relaxed);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }

        // Peak concurrency must never exceed the permit count.
        assert!(
            peak.load(Ordering::Relaxed) <= 2,
            "peak concurrency {} exceeded permit count 2",
            peak.load(Ordering::Relaxed)
        );
    }
}
