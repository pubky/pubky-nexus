# Development

Setting up a local Nexus, running the tests and the benchmarks.

## Setting up the environment

To get started with Nexus, first set up the required databases: Neo4j and Redis.

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

3. Optionally, populate the Neo4j database with initial mock data. See [Running the tests](#running-the-tests) for loading the mock data.

4. Run the Nexus service:

```bash
# Run with defaults values. The config file will default from $HOME/.pubky-nexus/config.toml
cargo run -p nexusd
# Run from config file
cargo run -p nexusd -- --config-dir="custom/config/folder"
# There is also an option to run services individually
# Useful to run a database clear command before start running the watcher
# cargo run -p nexusd -- db clear --yes
cargo run -p nexusd -- watcher
cargo run -p nexusd -- api
```

5. **Access Redis and Neo4j UIs and Swagger endpoint**:
   - Swagger UI: [http://localhost:8080/swagger-ui](http://localhost:8080/swagger-ui)
   - Redis Insight: [http://localhost:5540/0/browser](http://localhost:5540/0/browser)
     - Note: on first run, an error popup is shown and a TOS popup. After you accept the TOS, the link will work.
   - Neo4J Browser: [http://localhost:7474/browser/](http://localhost:7474/browser/)

## Running the tests

Running tests requires setting up mock data (`docker/test-graph/mocks`) into Neo4j and Redis.

Use the `db` command to load the mock data:

```bash
# If you're using podman instead of docker, set this env variable before importing mock data
# export CONTAINER_RUNTIME=podman
cargo run -p nexusd -- db mock
```

Then to run the tests:

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
