//! Five `/events/` polls of one homeserver through the [`Watcher::homeserver`] builder.
//!
//! Polls the public staging homeserver, prints each event, and reads each `PUT`'s resource.
//! Each run returns the cursor to start the next one from; an application would persist it.
//! From the workspace root:
//!
//! ```bash
//! cargo run -p pubky-watcher --example poll_homeserver_builder
//! ```
//!
//! The crate logs with `tracing`. This example prints instead of installing a subscriber.

use async_trait::async_trait;
use pubky::{EventCursor, PubkyResource, PublicKey};
use pubky_watcher::{
    EventHandler, EventMethod, HomeserverEvent, ResourceReader, Watcher, WatcherClient,
};
use tokio::sync::watch;

/// This example's error: any error, boxed. An application would use its own type.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

const STAGING_HOMESERVER: &str = "ufibwbmed6jeq9k4p583go95wofakh9fwpp4k734trq79pd9u1uy";
const POLL_TICKS: usize = 5;
const EVENTS_PER_TICK: u16 = 8;
/// How much of a resource to print.
const PRINTED_BYTES: usize = 200;

/// Prints each event, and the start of each written resource.
struct ResourcePrintingHandler {
    client: WatcherClient,
}

#[async_trait]
impl EventHandler<HomeserverEvent, BoxError> for ResourcePrintingHandler {
    async fn handle(&self, event: &HomeserverEvent) -> Result<(), BoxError> {
        println!("{} {}", event.method, event.uri);
        if event.method != EventMethod::Put {
            return Ok(());
        }

        let resource: PubkyResource = event.uri.parse()?;
        match self.client.get_resource(&resource).await {
            Ok(body) => {
                let shown = &body[..body.len().min(PRINTED_BYTES)];
                println!("  {} bytes: {}", body.len(), String::from_utf8_lossy(shown));
            }
            // The resource may have been deleted since, or be larger than a JSON resource.
            Err(error) => println!("  {error}"),
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let homeserver = PublicKey::try_from_z32(STAGING_HOMESERVER)?;
    let client = WatcherClient::mainnet()?;

    let watcher = Watcher::homeserver(client.clone(), homeserver)
        .handler(ResourcePrintingHandler { client })
        .events_limit(EVENTS_PER_TICK)
        .build(shutdown_rx)?;

    let mut cursor = EventCursor::new(0);
    for tick in 1..=POLL_TICKS {
        println!("Tick {tick}/{POLL_TICKS}, from cursor {}", cursor.id());
        let outcome = watcher.run(cursor).await?;
        cursor = outcome.cursor;
    }

    println!("next cursor: {}", cursor.id());
    Ok(())
}
