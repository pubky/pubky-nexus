# Development

Setting up a local Nexus, running the tests and the benchmarks.

## Prerequisites

- **Rust:** the toolchain pinned in `rust-toolchain.toml`. `rustup` installs it on the first `cargo` command.
- **Docker** (or Podman) with Compose, for Neo4j, Redis and, for the watcher tests, Postgres.
- **cargo-nextest:** `cargo install cargo-nextest --locked`. The tests run only through it: `.config/nextest.toml` keeps apart the tests that share Redis keys, which `cargo test` would run at the same time.
- **ImageMagick:** the media code calls its `convert` and `identify` commands, so the `nexus-webapi` media tests fail without it.

## Setting up the environment

1. Clone the repository and navigate to the project directory.
2. Copy the environment template and set up the Docker environment:

```bash
cd docker
cp .env-sample .env

# Lean stack: Neo4j + Redis + Redis Insight
docker compose up -d

# With Postgres (for watcher tests)
docker compose --profile tests up -d
```

To always start Postgres without passing `--profile tests`, uncomment `COMPOSE_PROFILES=tests` in `.env`.

3. Run Nexus:

```bash
# The API, the watcher and the scheduled jobs. Reads ~/.pubky-nexus/config.toml,
# which is created from nexusd/default.config.toml when it is missing
cargo run -p nexusd
# The same, with the config.toml in another folder
cargo run -p nexusd -- --config-dir="custom/config/folder"
# One service at a time
cargo run -p nexusd -- watcher
cargo run -p nexusd -- api
```

With the default config, the watcher indexes the live Synonym homeserver (`[watcher] homeserver`, with `[stack.net] testnet = false`) into your local databases. At start it also stores the `[features]` switches, which turn the trust filter off by default. Both change data the tests rely on, so load the mock data again before running the tests (next section).

4. **Access Redis and Neo4j UIs and Swagger endpoint**:
   - Swagger UI: [http://localhost:8080/swagger-ui](http://localhost:8080/swagger-ui)
   - Redis Insight: [http://localhost:5540/0/browser](http://localhost:5540/0/browser)
     - Note: on first run, an error popup is shown and a TOS popup. After you accept the TOS, the link will work.
   - Neo4J Browser: [http://localhost:7474/browser/](http://localhost:7474/browser/)

## Running the tests

The tests read the mock data in `docker/test-graph/mocks`. Load it into Neo4j and Redis before the first run, and again after running the watcher against a real homeserver:

```bash
# If you're using podman instead of docker, set this env variable before importing mock data
# export CONTAINER_RUNTIME=podman
cargo run -p nexusd -- db mock
# Only one of the two databases
cargo run -p nexusd -- db mock --mock-type graph   # or redis
```

> ⚠️ `db mock` and `db clear --yes` delete everything first: every node in the Neo4j graph and every key in the Redis database (`FLUSHDB`), including the watcher's cursors and retry queue. They act on the databases in `~/.pubky-nexus/config.toml`, or in the folder given with `--config-dir`. Check that both point at `localhost` before running either.

Then run the tests, in this order:

```bash
cargo nextest run -p nexus-common --no-fail-fast

cargo nextest run -p nexus-webapi --no-fail-fast

# nexus-watcher tests need Postgres (docker compose --profile tests up -d) and
# TEST_PUBKY_CONNECTION_STRING from docker/.env-sample
# export TEST_PUBKY_CONNECTION_STRING=postgres://test_user:test_pass@localhost:5432/postgres?pubky-test=true
cargo nextest run -p nexus-watcher --no-fail-fast

# nexusd trust-rank tests require the GDS plugin baked into the neo4j image
# (docker/neo4j/Dockerfile); the docker compose stack builds it automatically.
cargo nextest run -p nexusd --no-fail-fast
```

Run the `nexusd` suite last. Its trust recompute rewrites the `trust` of every user in the graph, and the other suites check graph state. If they fail after it, load the mock data again.

To test specific feature(s):

```bash
cargo nextest run -p nexus-watcher files::create --no-fail-fast
```

## Benchmarks

If you want to see the performance of the server you can run the benchmarks

```bash
# cargo bench -p nexus-webapi [--bench <endpoint>]
cargo bench -p nexus-webapi
# or if you want specific endpoint
cargo bench -p nexus-webapi --bench user
```

## Troubleshooting

If tests or the development environment seem out of sync, follow the [Running the tests](#running-the-tests) steps to reload the mock data.
