# Nexus Watcher

Nexus Watcher is a service that monitors events from a Pubky homeserver and updates the Nexus databases accordingly.  
It polls for events from the `/events` endpoint of a homeserver and processes them ensuring that the graph database (Neo4j) and Redis indexes remain synchronized.

## Features

- **Event Processing:**  
  Processes various types of events such as posts, bookmarks, follows, mutes, tags, and user profile updates using [`pubky-app-specs`](https://github.com/pubky/pubky-app-specs) object builder.

- **Retry Mechanism:**  
  Supports retry logic for events that fail to index due to missing dependencies or other transient errors

- **Integration with Nexus Common:**  
  Leverages shared components from the `nexus-common` crate for configuration, database access, logging, and stack management

- **Configurable and Extensible:**  
  Provides a builder API to configure service parameters such as the homeserver Pubky ID, database settings, logging level, testnet mode, and more

- **Comprehensive Testing:**  
  Comes with an extensive test suite covering all event types and error conditions

## Quick Examples

The main entry point is available via the builder in the `nexus_watcher::builder` module. For example, you can start the watcher using:

```rust
use nexus_watcher::builder::NexusWatcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    NexusWatcher::builder().run().await
}
```

Alternatively, if you prefer to load the configuration from a file:

```rust
use nexus_watcher::builder::NexusWatcher;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    NexusWatcher::start_from_path(PathBuf::from("path/to/config/folder")).await?;
    Ok(())
}
```

## License

This project is licensed under the MIT License.
