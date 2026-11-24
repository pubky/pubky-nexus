//! One read of three users' `/events-stream`s on one homeserver through the
//! [`Watcher::key_stream`] builder, with one cursor per user.
//!
//! From the workspace root:
//!
//! ```bash
//! cargo run -p pubky-watcher --example poll_key_stream_builder
//! ```

use async_trait::async_trait;
use pubky::{EventCursor, PublicKey};
use pubky_watcher::{EventHandler, HsStreamEvent, Watcher, WatcherClient};
use tokio::sync::watch;

/// This example's error: any error, boxed. An application would use its own type.
type BoxError = Box<dyn std::error::Error + Send + Sync>;

const STAGING_HOMESERVER: &str = "ufibwbmed6jeq9k4p583go95wofakh9fwpp4k734trq79pd9u1uy";
const EVENTS_PER_USER: u16 = 8;
const USER_KEYS: [&str; 3] = [
    "wws1gjzowkp3aeacjku1mu9dewzor9mr6secjfreeskgz8as48gy",
    "5a1diz4pghi47ywdfyfzpit5f3bdomzt4pugpbmq4rngdd4iub4y",
    "68rkfi1d78baobycj6w4b7dga43o8qtnuhubban5at6qywrieb5y",
];

struct PrintingHandler;

#[async_trait]
impl EventHandler<HsStreamEvent, BoxError> for PrintingHandler {
    async fn handle(&self, event: &HsStreamEvent) -> Result<(), BoxError> {
        println!(
            "{} {} cursor={}",
            event.event_type,
            event.uri(),
            event.cursor
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let client = WatcherClient::mainnet()?;
    let homeserver = PublicKey::try_from_z32(STAGING_HOMESERVER)?;
    let users = USER_KEYS
        .into_iter()
        .map(|key| PublicKey::try_from_z32(key).map(|user| (user, EventCursor::new(0))))
        .collect::<Result<Vec<_>, _>>()?;
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let watcher = Watcher::key_stream(client, homeserver, users)
        .handler(PrintingHandler)
        .events_limit(EVENTS_PER_USER)
        .path("/pub/")
        .build(shutdown_rx)?;

    let outcome = watcher.run().await?;
    for (user, cursor) in outcome.cursors {
        println!("{}: next cursor {}", user.z32(), cursor.id());
    }
    Ok(())
}
