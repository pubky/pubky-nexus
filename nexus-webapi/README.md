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

## Light Mode

With `mode = "light"` under `[stack]` (see the [root README](../README.md#-light-mode)), the API serves the social graph but none of the content people wrote or uploaded:

- Posts are served without `content`, users without `name`, `bio`, `links` and `status`, files without `name` or `urls`. Each carries what a client needs to fetch the content from its homeserver: `content_hash` and `author_homeserver` on posts, `profile_hash` and `homeserver` on users, `src` and `blocked` on files.
- Endpoints that need that content answer `501` with `{"error": "unavailable in light mode"}`: `/v0/search/posts/by_content`, `/v0/search/users/by_name/{prefix}`, `/v0/stream/users/username`, `/v0/stream/posts` and `/v0/stream/posts/keys` with `source=collection` or `source=post_collections`, and every `/static/...` route. Call `Error::require_full_mode()` first in a new endpoint that needs content.
- `/v0/info` reports the `mode`, and the Swagger UI opens with a guide for light clients.
- On start, `NexusApiBuilder::start` refuses a database indexed in the other mode (`StackManager::ensure_mode_lock`).

Light-mode tests live in `tests/light`. They set up a light stack and drive a standalone router, so they need `cargo nextest`, which runs each test in its own process.

## Advanced Configuration

For more advanced scenarios, use the builder pattern via `NexusApi::builder()` to adjust parameters such as the public address, logging level, file paths, and database settings

## License

This project is licensed under the MIT License.
