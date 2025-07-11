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

Both `api` and `watcher` binaries take an optional `config` argument:

```bash
# Expects api-config.toml in the given path
cargo run --bin api_example -- --config=test_path

# Expects watcher-config.toml in the given path
cargo run --bin watcher_example -- --config=test_path
```

If the `test_path` contains no valid config file, a default `config.toml` will be created and used instead.
