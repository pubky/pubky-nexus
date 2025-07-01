# Examples

Example usage of crates `nexus-webapi` and `nexus-watcher`:

- **`api`** – A Nexus API web server exposing HTTP endpoints to query the local graph database or in-memory cache
- **`watcher`** – A background service that fetches homeserver events and ingests them into the graph database or cache

## Quickstart

From the project root (`/pubky-nexus`);

```bash
# Run the API server (serves on localhost:8081 by default)
cargo run --bin api_example

# Run the watcher
cargo run --bin watcher_example
```

## Configuration & Flags

Both `api` and `watcher` binaries include a compile-time switch in `main.rs`:

```rust
// Toggle between embedded/default config vs. file-based config
const FROM_FILE: bool = false;
```

When `FROM_FILE` is false, you must supply all service configuration parameters at runtime; when `FROM_FILE` is true, the application will instead load its settings from a `config.toml` file
