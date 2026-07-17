//! Stubs shared by the `jobs` unit tests. Each is a dumb canned responder: the
//! behaviour under test lives in the scheduler, never in here.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::error::Error;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

use super::error::{LockError, LockResult};
use super::lock::RunLock;
use super::scheduler::NowFn;
use super::Job;

/// A wall clock anchored at `epoch` that tracks tokio's virtual time, paired
/// with a step handle. Storing seconds into the handle moves the wall clock
/// *without* advancing tokio's monotonic clock: forward is a host suspend,
/// backward is an NTP correction or a snapshot restore.
///
/// `epoch` is pinned by the caller so a fire time sits a known distance away,
/// rather than wherever the real `Utc::now()` happens to fall.
pub(super) fn steppable_clock(epoch: DateTime<Utc>) -> (NowFn, Arc<AtomicI64>) {
    let virt0 = tokio::time::Instant::now();
    let skew = Arc::new(AtomicI64::new(0));
    let handle = Arc::clone(&skew);
    let now_fn: NowFn = Arc::new(move || {
        epoch
            + chrono::Duration::from_std(virt0.elapsed()).expect("virtual elapsed must fit")
            + chrono::Duration::seconds(skew.load(Ordering::SeqCst))
    });
    (now_fn, handle)
}

/// What [`FakeLock`]'s `acquire` reports, fixed per instance.
pub(super) enum AcquireOutcome {
    /// Always granted.
    Granted,
    /// Always denied — another run holds the lease.
    Denied,
    /// Always a backend failure.
    Fails,
    /// Granted on even calls, denied on odd, so one run exercises both branches.
    Alternating,
}

/// The [`RunLock`] stub: a canned `acquire` outcome plus counters and the last
/// job name seen on each call.
pub(super) struct FakeLock {
    outcome: AcquireOutcome,
    acquires: AtomicU32,
    releases: AtomicU32,
    acquired_with: Mutex<Option<String>>,
    released_with: Mutex<Option<String>>,
}

impl FakeLock {
    pub(super) fn new(outcome: AcquireOutcome) -> Arc<Self> {
        Arc::new(Self {
            outcome,
            acquires: AtomicU32::new(0),
            releases: AtomicU32::new(0),
            acquired_with: Mutex::new(None),
            released_with: Mutex::new(None),
        })
    }

    /// Calls to `acquire`, including denied and failed ones.
    pub(super) fn acquires(&self) -> u32 {
        self.acquires.load(Ordering::SeqCst)
    }

    /// Calls to `unlock`.
    pub(super) fn releases(&self) -> u32 {
        self.releases.load(Ordering::SeqCst)
    }

    /// Job name the last `acquire` was handed.
    pub(super) fn acquired_with(&self) -> Option<String> {
        self.acquired_with.lock().unwrap().clone()
    }

    /// Job name the last `unlock` was handed.
    pub(super) fn released_with(&self) -> Option<String> {
        self.released_with.lock().unwrap().clone()
    }
}

#[async_trait]
impl RunLock for FakeLock {
    fn new_token(&self) -> String {
        "token".to_string()
    }

    async fn acquire(&self, job: &str, _token: &str) -> LockResult<bool> {
        *self.acquired_with.lock().unwrap() = Some(job.to_string());
        let call = self.acquires.fetch_add(1, Ordering::SeqCst);
        match self.outcome {
            AcquireOutcome::Granted => Ok(true),
            AcquireOutcome::Denied => Ok(false),
            AcquireOutcome::Fails => Err(LockError("backend down".into())),
            AcquireOutcome::Alternating => Ok(call.is_multiple_of(2)),
        }
    }

    async fn unlock(&self, job: &str, _token: &str) -> LockResult<()> {
        *self.released_with.lock().unwrap() = Some(job.to_string());
        self.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Counts its runs. The default [`Job`] stub, for tests where the body doesn't
/// matter; `name` is a parameter because it keys the lock and the metric labels.
pub(super) struct CountingJob {
    name: &'static str,
    runs: AtomicU32,
}

impl CountingJob {
    pub(super) fn new(name: &'static str) -> Self {
        Self {
            name,
            runs: AtomicU32::new(0),
        }
    }

    /// Completed runs.
    pub(super) fn runs(&self) -> u32 {
        self.runs.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Job for CountingJob {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Panics on every fire.
pub(super) struct PanicJob;

#[async_trait]
impl Job for PanicJob {
    fn name(&self) -> &'static str {
        "panicky"
    }

    async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        panic!("boom");
    }
}

/// Marks itself started, then blocks forever on a gate nothing signals.
pub(super) struct BlockingJob {
    started: AtomicU32,
    completed: AtomicU32,
    gate: Notify,
}

impl BlockingJob {
    pub(super) fn new() -> Self {
        Self {
            started: AtomicU32::new(0),
            completed: AtomicU32::new(0),
            gate: Notify::new(),
        }
    }

    /// Runs that reached the gate.
    pub(super) fn started(&self) -> u32 {
        self.started.load(Ordering::SeqCst)
    }

    /// Runs that got past the gate — stays 0 unless the run is left to hang.
    pub(super) fn completed(&self) -> u32 {
        self.completed.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Job for BlockingJob {
    fn name(&self) -> &'static str {
        "blocking"
    }

    async fn run(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.started.fetch_add(1, Ordering::SeqCst);
        self.gate.notified().await; // never signaled
        self.completed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
