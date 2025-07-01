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

## Advanced Configuration

For more advanced scenarios, use the builder pattern via `NexusApi::builder()` to adjust parameters such as the public address, logging level, file paths, and database settings

## License

This project is licensed under the MIT License.
