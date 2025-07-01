# Nexus Common

Nexus Common is a foundational crate that provides shared configuration, database connectors, data models and media processing utilities used throughout the Nexus indexer stack.

## Overview

The `nexus-common` crate offers:

- **Configuration Management:** TOML-based loader with default file generation, home‑dir expansion, and asynchronous loading

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

## Module Overview

### Configuration Management
- Module: `config/`
- Features: `ConfigLoader`, default templates, home-dir expansion, async loading

### Database Connectivity
- Module: `db/`
- Features: `Neo4jConnector`, `RedisConnector`, `get_neo4j_graph()`, `get_redis_conn()`, error handling

### Data Models

The data models cover the core domain entities and their graph/cache operations:

- **Files:** Representing file details, blobs, and URLs
- **Users:** Including user details, counts, relationships, and search capabilities
- **Posts:** Covering post details, counts, relationships, bookmarks, and views
- **Tags:** Managing tag details, global taggers, and search operations
- **Follows:** Handling follower, following, and friends relationships
- **Notifications:** Representing user notifications

### Shared Types

The crate provides common types and utilities (`types/`) that are used across different modules, such as:

- `Pagination` for paginated queries
- `Timeframe` for filtering data based on time ranges
- `StreamSorting` for ordering streams

### Media Processing
- Module: `media/`
- Features: `ImageProcessor`, `VideoProcessor`, `FileVariant`, and `VariantController` for automated processing pipelines

MIME-type management and storage directory configuration

## Getting Started

To add `nexus-common` to your project, include it in your `Cargo.toml` dependencies:

```bash
cargo add nexus-common
```

## Quick Examples

### Configuration Loading

Below is an example demonstrating how to load a configuration using the provided loader trait:

```rust
use nexus_common::config::{ConfigLoader, DaemonConfig};
use std::path::Path;
use nexus_common::types::DynError;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let cfg: DaemonConfig = DaemonConfig::read_config_file(
        config::expand_home_dir("~/.pubky-nexus".into())
    ).await?;
    println!("Loaded config: {:#?}", cfg);
    Ok(())
}
```

### Database Connectivity

Below is an example demonstrating how to get the connectors of the data bases:

```rust
use nexus_common::db::{Neo4jConnector, RedisConnector, get_neo4j_graph, get_redis_conn};
use nexus_common::db::Neo4JConfig;
use nexus_common::types::DynError;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    // Initialize connectors (once per app)
    Neo4jConnector::init(Neo4JConfig::default()).await?;
    RedisConnector::init("redis://127.0.0.1:6379").await?;

    // Use helper functions
    let graph = get_neo4j_graph()?;
    let mut redis_conn = get_redis_conn().await?;
    Ok(())
}
```

### Data Models & Caching

Demonstrates cache-first retrieval of domain entities, attempting to load from Redis and falling back to Neo4j if not found

```rust
use nexus_common::models::user::UserDetails;
use nexus_common::types::DynError;
use nexus_common::{StackManager, StackConfig};

#[tokio::main]
async fn main() -> Result<(), DynError> {
    StackManager::setup("common-example", StackConfig::default()).await?
    // Cache-first: Redis -> Neo4j fallback
    if let Some(user) = UserDetails::get_by_id("some_user_id").await? {
        println!("User: {}", user.name);
    }
    Ok(())
}
```

## Contributing

Contributions to `nexus-common` are welcome! Please open issues or submit pull requests on the project's repository. Follow the established coding conventions and include tests for new features.

## License

This project is licensed under the MIT License.
