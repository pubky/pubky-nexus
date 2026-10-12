# Nexus API

Nexus API is a RESTful API server built on top of Axum that serves as the core backend for Pubky App. It integrates with two databases: Neo4j graph database and Redis cache, and it supports distributed tracing and interactive API documentation.

## Overview

Nexus API is designed to handle endpoints related to:

- **Users:** Retrieving user profiles, relationships, and streams.
- **Posts:** Managing post details, counts, bookmarks, and tag-related operations.
- **Files:** Serving static files and file details.
- **Tags:** Searching and managing tags for posts and users.
- **Notifications:** Handling user notifications.
- **Streams:** Providing real-time streams for posts and user data.

The crate leverages the shared `nexus_common` library for database interactions and common types. Its modular architecture ensures that each responsibility is neatly encapsulated within dedicated modules.

## Key Features

- **Robust Routing:** Uses Axum’s routing system to define clear API endpoints across multiple versions.
- **Modular Design:** Organized into modules like `builder`, `config`, `error`, `mock`, `models`, and `routes`.
- **Observability:** Integrated OpenTelemetry tracing and automatically generated OpenAPI documentation via Swagger UI.
- **High Performance:** Includes extensive testing and benchmarking to ensure optimal performance.
- **Flexible Configuration:** Configurable via `toml` files with sensible defaults provided by the `ApiConfig` struct.

## Event Log

`GET /v0/events` replays every event this Nexus has indexed, in order, as plain text: one `PUT|DEL pubky://<author>/<path>` line per event, then `cursor: <n>`. Another indexer can follow it to learn which records exist, then fetch each record from its author's homeserver.

To follow only some authors (e.g. a Nexus that indexes part of the network), use `POST /v0/events/by_users` with `{"users": ["<pk>", ...], "cursor": 0, "limit": 500}` (up to 1,000 Pubky ids). `GET /v0/events` rejects a `users` parameter with a 400 pointing here.

A filtered request scans at most 10,000 log lines, so a page can hold fewer than `limit` lines, even none, before the log ends. Keep paging with the returned `cursor` (the position after the last line scanned); the end is reached when it equals the cursor you sent. The route is in the expensive rate-limit bucket.

Two headers tell a follower when its stored cursor no longer fits the log, in which case it restarts from 0:

- `x-events-epoch` identifies the log. It is created with the first stored event and changes when the log is recreated (e.g. after `db clear`). A change from one value to another means restart. While the header is absent (an older log, before its next stored event), keep the epoch you stored; an epoch appearing where there was none is not a change.
- `x-events-length` is the log's length when the request started. If the cursor you **sent** is past it, the log was truncated or restored from an older backup. Don't compare it with the returned cursor: events appended during the request can make the returned cursor larger.

Operators restoring Redis from a backup should then give the log a new epoch, e.g. `SET Event:Epoch <current time in ms>`, so every follower sees it change and restarts. Set a new value rather than deleting the key, so there is never a gap without one.

## Installation

To add Nexus API to your project, include it in your `Cargo.toml` dependencies:

```bash
cargo add nexus-webapi
```

## Quick Examples

Below is a simple example to start the Nexus API server:

```rust
use nexus_webapi::builder::NexusApi;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build and run the Nexus API server
    NexusApi::builder().run().await?;
    Ok(())
}
```

Alternatively, if you prefer to load the configuration from a file:

```rust
use nexus_watcher::builder::NexusWatcher;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    NexusApi::start_from_path(PathBuf::from("path/to/config/folder")).await?;
    Ok(())
}
```

## Advanced Configuration

For more advanced scenarios, use the builder pattern via `NexusApi::builder()` to adjust parameters such as the public address, logging level, file paths, and database settings

## License

This project is licensed under the MIT License.
