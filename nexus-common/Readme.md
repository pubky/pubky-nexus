# Nexus Common

Nexus Common is a foundational crate that provides shared configuration, database connectors, data models and media processing utilities used throughout the Nexus indexer stack.

## Overview

The `nexus-common` crate offers:

- **Configuration Management:**  
  Load configuration files from TOML using a trait-based loader.

- **Database Connectivity:**  
  Connect to Neo4j and Redis using dedicated connectors. Use functions such as `get_neo4j_graph()` and `get_redis_conn()` to obtain connections for executing queries and handling errors.

- **Data Models:**  
  Define core entities such as Files, Users, Posts, Tags, Notifications, and Follows. Each model provides functionality for caching, indexing, and graph operations.

- **Indexing and Caching:**  
  Utilities to index and retrieve records from Redis. Includes support for pagination, lexicographical range queries, and sorted sets.

- **Shared Types and Traits:**  
  Common types such as `Pagination`, `Timeframe`, and `StreamSorting` along with a collection of traits are provided to ensure a consistent interface across the Nexus backend.

- **Media Processing:**  
  Process media files with support for image and video variant creation. The crate includes processors (like `ImageProcessor` and a prototype `VideoProcessor`) that generate different file variants (e.g. main, feed, small) and manage content types.

This crate is designed as a backbone for other services (e.g, homeserver watcher and API) crates in the Nexus stack, enabling consistent access to core functionalities and shared data structures.

## Getting Started

To add `nexus-common` to your project, include it in your `Cargo.toml` dependencies:

```bash
cargo add nexus-common
```

## Usage

Below is an example demonstrating how to load a configuration using the provided loader trait:

```rust
use nexus_common::config::ConfigLoader;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ConfigLoader::load(Path::new("config.toml")).await?;
    println!("Loaded configuration: {:?}", config);
    Ok(())
}
```

## Database Connectors

The crate includes connectors for both Neo4j and Redis:

- To get a Neo4j connection, use `get_neo4j_graph()`.
- To get a Redis connection, use `get_redis_conn()`.

## Data Models

The data models cover various aspects of the application:

- **Files:** Representing file details, blobs, and URLs.
- **Users:** Including user details, counts, relationships, and search capabilities.
- **Posts:** Covering post details, counts, relationships, bookmarks, and views.
- **Tags:** Managing tag details, global taggers, and search operations.
- **Follows:** Handling follower, following, and friends relationships.
- **Notifications:** Representing user notifications.

## Shared Types

The crate provides common types and utilities that are used across different modules, such as:

- `Pagination` for paginated queries.
- `Timeframe` for filtering data based on time ranges.
- `StreamSorting` for ordering streams.

## Contributing

Contributions to `nexus-common` are welcome! Please open issues or submit pull requests on the project's repository. Follow the established coding conventions and include tests for new features.

## License

This project is licensed under the MIT License.
