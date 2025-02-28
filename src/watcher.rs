use pubky_nexus::_watcher::NexusWatcher;

/// Watches over a homeserver `/events` and writes into the Nexus databases
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    NexusWatcher::builder().run().await
}
