# Nexus Watcher

Nexus Watcher is a service that monitors events from a Pubky homeserver and updates the Nexus databases accordingly.  
It polls for events from the `/events` endpoint of a homeserver and processes them ensuring that the graph database (Neo4j) and Redis indexes remain synchronized.

## Features

- **Event Processing:**  
  Processes various types of events such as posts, bookmarks, follows, tags, and user profile updates using [`pubky-app-specs`](https://github.com/pubky/pubky-app-specs) object builder.

- **Retry Mechanism:**  
  Supports retry logic for events that fail to index due to missing dependencies or other transient errors

- **Integration with Nexus Common:**  
  Leverages shared components from the `nexus-common` crate for configuration, database access, logging, and stack management

- **Configurable and Extensible:**  
  Provides a builder API to configure service parameters such as the homeserver Pubky ID, database settings, logging level, and more.

- **Comprehensive Testing:**  
  Comes with an extensive test suite covering all event types and error conditions

## Light Mode

With `mode = "light"` under `[stack]` (see the [root README](../README.md#-light-mode)), the watcher still downloads and validates every event, but stores only links:

- **Posts:** the content is read to index mentions and detect edits (by `content_hash`), then dropped. Collection posts are indexed without COLLECTED edges.
- **Profiles:** name, bio, links and status are dropped, and names are not indexed for search. `image` and `profile_hash` are kept.
- **Files:** the bytes are never downloaded. A slim record is kept: `src`, content type, size, and `blocked` when `src` is on a blacklisted homeserver.
- **Homeservers:** after each poll, whether the homeserver answered is recorded at `Hs:Reachability:<id>` for the API to serve.

On start, `NexusWatcherBuilder::start` refuses a database indexed in the other mode (`StackManager::ensure_mode_lock`). Light-mode tests live in `tests/event_processor/light` and use `WatcherTest::setup_light`; they need `cargo nextest`, which runs each test in its own process.

## Quick Examples

The main entry point is available via the builder in the `nexus_watcher::service` module. For example, you can start the watcher using:

```rust
use nexus_watcher::service::NexusWatcher;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    NexusWatcher::builder().run().await
}
```

Alternatively, if you prefer to load the configuration from a file:

```rust
use nexus_watcher::service::NexusWatcher;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    NexusWatcher::start_from_path(PathBuf::from("path/to/config/folder")).await?;
    Ok(())
}
```

## License

This project is licensed under the MIT License.
