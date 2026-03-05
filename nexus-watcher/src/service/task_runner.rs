use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::watch::{self, Receiver};
use tokio::task::JoinSet;
use tokio::time::{Duration, MissedTickBehavior};
use tracing::{debug, error, info};

use nexus_common::types::DynError;

/// A boxed async function that can be called repeatedly to produce a future.
///
/// Each invocation represents one "tick" of a periodic task.
pub type TaskFn =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), DynError>> + Send>> + Send + Sync>;

/// A periodic task to be run by [`run_periodic_tasks`].
///
/// Each task has a name (for logging), an interval in milliseconds, and an
/// async function that will be called on every tick of its interval timer.
pub struct PeriodicTask {
    pub name: String,
    pub interval_ms: u64,
    pub task_fn: TaskFn,
}

impl PeriodicTask {
    /// Create a [`PeriodicTask`] from a name, interval, and any `Fn` that returns a future.
    ///
    /// This handles the `Arc` + `Box::pin` wrapping so callers don't have to.
    ///
    /// ```ignore
    /// PeriodicTask::new("my-task", 5000, move || {
    ///     let runner = runner.clone();
    ///     async move { runner.do_work().await }
    /// })
    /// ```
    pub fn new<F, Fut>(name: impl Into<String>, interval_ms: u64, f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), DynError>> + Send + 'static,
    {
        Self {
            name: name.into(),
            interval_ms,
            task_fn: Arc::new(move || Box::pin(f())),
        }
    }
}

/// Runs a set of [`PeriodicTask`]s concurrently, each on its own interval tick.
///
/// Each task runs in a loop that:
/// 1. Waits for the next interval tick (using the task's own `interval_ms`)
/// 2. Calls the task's async function
/// 3. Checks for shutdown or cancellation signals
///
/// If any task **panics**, an internal cancellation signal is sent so that all
/// other tasks stop after completing their current iteration. The function
/// then waits for all tasks to finish before returning.
///
/// The `shutdown_rx` channel provides an external shutdown signal (e.g. Ctrl-C).
///
/// # Returns
///
/// A `Vec<TaskResult>` with one entry per task, indicating how it finished.
pub async fn run_periodic_tasks(
    tasks: Vec<PeriodicTask>,
    shutdown_rx: Receiver<bool>,
) -> Vec<TaskResult> {
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let mut join_set = JoinSet::new();

    for task in tasks {
        let mut shutdown = shutdown_rx.clone();
        let mut cancel = cancel_rx.clone();
        let name = task.name.clone();
        let task_fn = task.task_fn;
        let interval_ms = task.interval_ms;

        join_set.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = shutdown.changed() => {
                        info!("Shutdown received, exiting '{name}' loop");
                        break;
                    }
                    _ = cancel.changed() => {
                        info!("Cancellation received, exiting '{name}' loop");
                        break;
                    }
                    _ = interval.tick() => {
                        debug!("Running task: {name}");
                        if let Err(e) = (task_fn)().await {
                            error!("Task '{name}' returned error: {e}");
                        }
                    }
                }
            }
            name
        });
    }

    // Drain the JoinSet. Each completed task is checked for panics;
    // a panic sends the cancellation signal so surviving siblings exit promptly.
    let mut results = Vec::with_capacity(join_set.len());
    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok(name) => results.push(TaskResult {
                name,
                outcome: TaskOutcome::Completed,
            }),
            Err(join_error) => {
                let name = format!("{join_error}");
                error!("Task panicked: {name}");
                let _ = cancel_tx.send(true);
                results.push(TaskResult {
                    name,
                    outcome: TaskOutcome::Panicked,
                });
            }
        }
    }

    results
}

/// The outcome of a single periodic task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOutcome {
    /// The task exited its loop normally (shutdown or cancellation signal).
    Completed,
    /// The task panicked, which triggered cancellation of siblings.
    Panicked,
}

/// Result of running a single periodic task.
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub name: String,
    pub outcome: TaskOutcome,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_shutdown_stops_all_tasks() {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let counter_a = Arc::new(AtomicU32::new(0));
        let counter_b = Arc::new(AtomicU32::new(0));

        let ca = counter_a.clone();
        let cb = counter_b.clone();

        let tasks = vec![
            PeriodicTask::new("task-a", 50, move || {
                ca.fetch_add(1, Ordering::SeqCst);
                async { Ok(()) }
            }),
            PeriodicTask::new("task-b", 50, move || {
                cb.fetch_add(1, Ordering::SeqCst);
                async { Ok(()) }
            }),
        ];

        // Send shutdown after a short delay
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            let _ = shutdown_tx.send(true);
        });

        let results = run_periodic_tasks(tasks, shutdown_rx).await;

        assert_eq!(results.len(), 2);
        for r in &results {
            assert_eq!(r.outcome, TaskOutcome::Completed);
        }
        // Both tasks should have ticked at least once
        assert!(counter_a.load(Ordering::SeqCst) >= 1);
        assert!(counter_b.load(Ordering::SeqCst) >= 1);
    }

    #[tokio::test]
    async fn test_panic_cancels_sibling_tasks() {
        let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let healthy_counter = Arc::new(AtomicU32::new(0));
        let hc = healthy_counter.clone();

        let tasks = vec![
            PeriodicTask::new("panicking-task", 50, || async {
                panic!("intentional test panic");
            }),
            PeriodicTask::new("healthy-task", 50, move || {
                hc.fetch_add(1, Ordering::SeqCst);
                async { Ok(()) }
            }),
        ];

        let results = run_periodic_tasks(tasks, shutdown_rx).await;

        let panicked_count = results.iter().filter(|r| r.outcome == TaskOutcome::Panicked).count();
        let completed_count = results.iter().filter(|r| r.outcome == TaskOutcome::Completed).count();

        assert_eq!(panicked_count, 1);
        assert_eq!(completed_count, 1);
    }

    #[tokio::test]
    async fn test_task_errors_do_not_cancel_siblings() {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let error_counter = Arc::new(AtomicU32::new(0));
        let healthy_counter = Arc::new(AtomicU32::new(0));
        let ec = error_counter.clone();
        let hc = healthy_counter.clone();

        let tasks = vec![
            PeriodicTask::new("erroring-task", 50, move || {
                ec.fetch_add(1, Ordering::SeqCst);
                async { Err("simulated error".into()) }
            }),
            PeriodicTask::new("healthy-task", 50, move || {
                hc.fetch_add(1, Ordering::SeqCst);
                async { Ok(()) }
            }),
        ];

        // Let them run for a bit then shut down
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let _ = shutdown_tx.send(true);
        });

        let results = run_periodic_tasks(tasks, shutdown_rx).await;

        // Both tasks should have completed normally
        for r in &results {
            assert_eq!(r.outcome, TaskOutcome::Completed);
        }

        // Both should have ticked multiple times
        assert!(error_counter.load(Ordering::SeqCst) >= 2);
        assert!(healthy_counter.load(Ordering::SeqCst) >= 2);
    }

    /// A slow task (one whose execution time exceeds the tick interval) should
    /// NOT queue up several back-to-back invocations.  With `MissedTickBehavior::Skip`
    /// the missed ticks are simply dropped, so the task runs roughly once per
    /// `max(interval, task_duration)` rather than bursting.
    #[tokio::test]
    async fn test_slow_task_skips_missed_ticks() {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        // Interval is 50 ms but the task sleeps for 150 ms,
        // so without Skip the counter would burst to catch up.
        let tasks = vec![PeriodicTask::new("slow-task", 50, move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(150)).await;
                Ok(())
            }
        })];

        // Let the task run for ~500 ms – enough for ~3 slow iterations.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let _ = shutdown_tx.send(true);
        });

        let results = run_periodic_tasks(tasks, shutdown_rx).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, TaskOutcome::Completed);

        let ticks = counter.load(Ordering::SeqCst);
        // With Skip behaviour and a 150 ms task on a 50 ms interval the task
        // should execute roughly every 150 ms.  Over 500 ms that is about 3–4
        // invocations.  Without Skip (Burst, the default) the counter would
        // race ahead to ~10.  We assert a reasonable upper bound.
        assert!(
            ticks <= 5,
            "expected at most 5 ticks (skip behaviour), but got {ticks}"
        );
        assert!(ticks >= 1, "task should have ticked at least once");
    }

    /// Verify that a fast task whose execution time is well below the
    /// interval still ticks at the expected cadence (Skip doesn't
    /// interfere with normal operation).
    #[tokio::test]
    async fn test_fast_task_ticks_normally_with_skip() {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        let tasks = vec![PeriodicTask::new("fast-task", 50, move || {
            c.fetch_add(1, Ordering::SeqCst);
            async { Ok(()) }
        })];

        // Run for ~250 ms ⇒ expect ~5 ticks (including the immediate first tick).
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let _ = shutdown_tx.send(true);
        });

        let results = run_periodic_tasks(tasks, shutdown_rx).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, TaskOutcome::Completed);

        let ticks = counter.load(Ordering::SeqCst);
        assert!(
            ticks >= 3,
            "expected at least 3 ticks for a fast 50 ms task over 250 ms, got {ticks}"
        );
    }
}
