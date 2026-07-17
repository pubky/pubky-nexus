use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Semaphore, SemaphorePermit};

use super::processors::MediaProcessorError;

/// How many requests may queue per permit before we shed instead of parking them.
const MAX_WAITING_PER_PERMIT: usize = 4;

/// Bounded concurrency gate for media subprocesses (ImageMagick/ffmpeg).
/// Created once from config; cloned cheaply (Arc<Semaphore> interior).
#[derive(Clone)]
pub struct MediaGate {
    semaphore: Arc<Semaphore>,
    /// Requests currently parked waiting for a permit.
    waiting: Arc<AtomicUsize>,
    /// Cap on parked requests. The semaphore's own wait list is unbounded, so without
    /// this a burst parks arbitrarily many requests, each holding a connection and task
    /// for the full acquire timeout — moving exhaustion from subprocesses to the queue.
    max_waiting: usize,
    /// How long a queued caller waits for a permit before shedding. With the queue
    /// bounded this is a backstop, not the primary shed mechanism.
    acquire_timeout: Duration,
}

impl MediaGate {
    /// Default wait for a permit before shedding.
    const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

    /// Create a gate that allows at most `permits` concurrent subprocesses.
    pub fn new(permits: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(permits)),
            waiting: Arc::new(AtomicUsize::new(0)),
            max_waiting: permits.max(1) * MAX_WAITING_PER_PERMIT,
            acquire_timeout: Self::DEFAULT_ACQUIRE_TIMEOUT,
        }
    }

    /// Override the acquire timeout (builder-style).
    pub fn with_acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    /// Override the queue cap (builder-style).
    pub fn with_max_waiting(mut self, max_waiting: usize) -> Self {
        self.max_waiting = max_waiting;
        self
    }

    /// Acquire a permit or shed with `AtCapacity`.
    pub async fn acquire(&self) -> Result<SemaphorePermit<'_>, MediaProcessorError> {
        // Fast path: capacity is free, so never touch the queue or the timer.
        if let Ok(permit) = self.semaphore.try_acquire() {
            return Ok(permit);
        }

        // Queue is already deep enough; shed now rather than park another request.
        let _waiting = WaitingGuard::enter(&self.waiting, self.max_waiting)?;

        match tokio::time::timeout(self.acquire_timeout, self.semaphore.acquire()).await {
            Ok(Ok(permit)) => Ok(permit),
            Ok(Err(_closed)) => Err(MediaProcessorError::AtCapacity), // semaphore closed (shouldn't happen)
            Err(_elapsed) => Err(MediaProcessorError::AtCapacity), // waited too long -> shed load
        }
    }
}

/// Counts a parked request, releasing on drop so cancelled requests don't leak a slot.
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

    use super::MediaGate;

    #[tokio_shared_rt::test(shared)]
    async fn test_contended_acquire_sheds_then_succeeds_once_released() {
        let gate = MediaGate::new(1).with_acquire_timeout(Duration::from_millis(50));

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
        let gate = MediaGate::new(1)
            .with_acquire_timeout(Duration::from_secs(30))
            .with_max_waiting(2);

        let _permit = gate.acquire().await.expect("first acquire should succeed");

        // Fill the queue with the two waiters it allows.
        let mut parked = Vec::new();
        for _ in 0..2 {
            let gate = gate.clone();
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
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_peak_concurrency_bounded() {
        // Uncapped queue: this test is about the permit count, not the shed threshold.
        let gate = MediaGate::new(2).with_max_waiting(usize::MAX);
        let concurrent = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        for _ in 0..10 {
            let gate = gate.clone();
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
