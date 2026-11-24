//! Five `/events/` polls of one homeserver through the low-level traits:
//! a [`TEventProcessor`] for one poll, run by a [`TEventProcessorRunner`].
//!
//! This is the path Nexus takes, with its own cursor store, retry queue and backoff. The
//! example keeps its cursor in memory and fails fast on any error. From the workspace root:
//!
//! ```bash
//! cargo run -p pubky-watcher --example poll_homeserver
//! ```
//!
//! The crate logs with `tracing`. This example prints instead of installing a subscriber.

use std::sync::Arc;

use async_trait::async_trait;
use pubky::PublicKey;
use pubky_watcher::{
    EventBatch, HomeserverEvent, HomeserverEventSource, RunOutcome, TEventProcessor,
    TEventProcessorRunner, WatcherClient,
};
use tokio::sync::{watch, Mutex};

/// This example's error: any error, boxed. An application would use its own type.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

const STAGING_HOMESERVER: &str = "ufibwbmed6jeq9k4p583go95wofakh9fwpp4k734trq79pd9u1uy";
const POLL_TICKS: usize = 5;
const EVENTS_PER_TICK: u16 = 8;

/// One poll of the homeserver: reads the batch after the cursor, prints each event, and
/// moves the cursor once the batch is done.
struct PollProcessor {
    homeserver: PublicKey,
    homeserver_id: String,
    source: Arc<dyn HomeserverEventSource>,
    cursor: Arc<Mutex<u64>>,
}

#[async_trait]
impl TEventProcessor<HomeserverEvent, BoxError> for PollProcessor {
    fn instance_name(&self) -> &'static str {
        "poll_homeserver"
    }

    fn homeserver_id(&self) -> &str {
        &self.homeserver_id
    }

    async fn handle(&self, event: &HomeserverEvent) -> Result<(), BoxError> {
        println!("{} {}", event.method, event.uri);
        Ok(())
    }

    async fn run_internal(self: Arc<Self>) -> Result<RunOutcome, BoxError> {
        let cursor = *self.cursor.lock().await;
        let lines = self
            .source
            .poll_events(&self.homeserver, cursor, EVENTS_PER_TICK)
            .await?;
        let batch = EventBatch::split(&lines);
        if !batch.has_events() {
            println!("No new events");
        }

        // `process_event_line` parses each line with `HomeserverEvent`'s `ParseFromLine`,
        // skips what it does not recognise, and hands the rest to `handle`.
        for line in &batch.event_lines {
            self.process_event_line(line).await?;
        }

        if let Some(next) = batch.cursor {
            *self.cursor.lock().await = next.parse()?;
            println!("cursor: {next}");
        }
        Ok(RunOutcome::Completed)
    }
}

/// Runs one [`PollProcessor`] per tick, for the staging homeserver only.
struct PollRunner {
    source: Arc<dyn HomeserverEventSource>,
    cursor: Arc<Mutex<u64>>,
    shutdown_rx: watch::Receiver<bool>,
}

#[async_trait]
impl TEventProcessorRunner<HomeserverEvent, BoxError> for PollRunner {
    type Error = BoxError;

    fn shutdown_rx(&self) -> watch::Receiver<bool> {
        self.shutdown_rx.clone()
    }

    async fn build(
        &self,
        hs_id: &str,
    ) -> Result<Arc<dyn TEventProcessor<HomeserverEvent, BoxError>>, BoxError> {
        Ok(Arc::new(PollProcessor {
            homeserver: PublicKey::try_from_z32(hs_id)?,
            homeserver_id: hs_id.to_string(),
            source: self.source.clone(),
            cursor: self.cursor.clone(),
        }))
    }

    async fn pre_run(&self) -> Result<Vec<String>, BoxError> {
        Ok(vec![STAGING_HOMESERVER.to_string()])
    }
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let runner = PollRunner {
        source: Arc::new(WatcherClient::mainnet()?),
        cursor: Arc::new(Mutex::new(0)),
        shutdown_rx,
    };

    for tick in 1..=POLL_TICKS {
        println!("Tick {tick}/{POLL_TICKS}");
        let stats = runner.run().await?.0;
        if stats.count_ok() != 1 {
            return Err("the poll failed".into());
        }
    }
    Ok(())
}
