![Integration Tests](https://github.com/pubky/pubky-nexus/actions/workflows/test.yml/badge.svg?branch=main)

# Pubky Nexus

Pubky Nexus is the central bridge connecting Pubky homeservers with [Pubky-App’s](https://github.com/pubky/pubky-app) social clients. By aggregating events from homeservers into a rich social graph, Nexus transforms decentralized interactions into a high-performance, fully featured social-media-like API. It's designed to support Social-Semantic-Graph (SSG) inference, and more.

## 🌟 Key Features

- **Real-time Social Graph Aggregation**: Nexus ingests events from multiple Pubky homeservers, generating a structured social graph in real time.
- **Full-Content Indexing**: Nexus serves content directly, improving latency and user experience. Clients do not need to locate homeservers to retrieve content unless they wish to perform content attestation. We also envision a light-weight Nexus mode that merely point clients to homeserver locations using pubky uris.
- **High Performance & Scalability**: Built in Rust, Nexus is optimized for speed and efficiency, handling complex social queries across distributed systems with minimal latency.
- **Powerful Social Semantic Graph**: Nexus supports SSG-based interactions, fostering secure and trusted connections between users.
- **Graph-Enhanced Search & Recommendations**: Nexus leverages Neo4j to offer deep insights, relationships, and recommendations based on the social graph.
- **Flexible Caching Layer**: A Redis cache accelerates access to common queries and minimizes database load, ensuring a smooth user experience. Most requests can be served in less than 1 ms at constant time complexity with respect number of users.
- **Rich Observability**: Easily explore the indexed data using [Redis Insight](https://redis.io/insight/) and visualize the graph database with [Neo4j Browser](https://browser.neo4j.io/).

## 🌐 Accessing the API

> ⚠️ **Warning**: The API is currently **unstable**. We are using the `/v0` route prefix while the API undergoes active development and changes. Expect potential breaking changes as we work toward stability.

Nexus provides a REST API, accessible via Swagger UI:

- **Staging API** (latest): [https://nexus.staging.pubky.app/swagger-ui/](https://nexus.staging.pubky.app/swagger-ui/)
- **Production API** (current): [https://nexus.pubky.app/swagger-ui/](https://nexus.pubky.app/swagger-ui/)

You can explore available endpoints, test queries, and view schema definitions directly within Swagger.

## 🏗️ Architecture Overview

Nexus is composed of several core components:

- **nexus-service**: The REST API server for handling client requests, querying databases, and returning responses to the Pubky-App frontend.
- **nexus-watcher**: The event aggregator that listens to homeserver events, translating them into social graph updates within the Nexus databases.
- **nexus-common**: A library crate containing common functionalities shared by `service` and `watcher`, including database connectors, models, and queries.
- **nexusd**: Manages the execution of Nexus components, with the capability to perform database migrations and reindexing when required

### Data Flow

![pubky-nexus-arch](docs/images/pubky-nexus-arch.png)

1. **Event Ingestion**: The watcher ingests events from Pubky homeservers and indexes them into our social graph.
2. **Indexing and Caching**: Relationships, common queries, and graph data are cached in Redis for high-speed access. Complex queries use Neo4j.
3. **API Responses**: The service server reads from these indexes to respond to client queries efficiently.

![pubky-nexus-graph](docs/images/pubky-nexus-graph.png)

Nexus graph schema.

## ⚙️ Setting Up the Development Environment

To get started with Nexus, first set up the required databases: Neo4j and Redis.

1. Clone the repository and navigate to the project directory.
2. Copy the environment template and set up the Docker environment:

```bash
cd docker
cp .env-sample .env
docker compose up -d
```

3. Optionally, populate the Neo4j database with initial mock data. Follow [Running Tests](#-running-tests) section about setting up mock data.

4. Run the Nexus service:

```bash
# Run with defaults values. The config file will default from $HOME/.pubky-nexus/config.toml
cargo run -p nexusd
# Run from config file
cargo run -p nexusd -- --config-dir="custom/config/folder"
# There is also an option to run services individually
# Useful to run a database clear command before start running the watcher
# cargo run -p nexusd -- db clear
cargo run -p nexusd -- watcher
cargo run -p nexusd -- api
```

5. **Access Redis and Neo4j UIs and Swagger endpoint**:
   - Swagger UI: [http://localhost:8080/swagger-ui](http://localhost:8080/swagger-ui)
   - Redis Insight: [http://localhost:8001/redis-stack/browser](http://localhost:8001/redis-stack/browser)
   - Neo4J Browser: [http://localhost:7474/browser/](http://localhost:7474/browser/)

## 📈 Observability

If you want to enable observability in Nexus, you can connect it to an OpenTelemetry exporter. Follow these steps:

1. Install Signoz locally – Follow the Signoz installation [guide](https://signoz.io/docs/install)
2. Configure the connection – In _config.toml_, uncomment the `otlp_endpoint` variable to point Nexus to the Signoz endpoint
3. Run Nexus – Start nexusd using the configured settings
4. Access the Signoz dashboard – Open http://localhost:3301 in your browser (allow some time for data to populate).

## 📦 Data Migrations

The Migration Manager is a purpose-built tool designed to simplify and standardize the process of performing data migrations in our backend system. It ensures a smooth transition during breaking changes to our data sources, such as Neo4j and Redis, by coordinating phased migrations with minimal disruption to the application. The manager tracks the status of each migration in the database, automates phase progression where possible, and provides a clear structure for developers to implement and manage migrations. This approach reduces the risk of data inconsistencies, ensures reliability during deployments, and keeps migration-related code isolated and easy to find.

### Understanding Migration Phases

The Migration Manager uses a phased approach to handle data migrations safely and systematically. Each phase serves a distinct purpose in transitioning data from the old source to the new source, ensuring consistency and minimal disruption. Here's an overview of the phases:

- **Dual Write**: During this phase, all writes to the old source are mirrored to the new source. This ensures that both sources remain synchronized during normal application operations. Developers invoke `MigrationManager::dual_write` in the application logic (preferrably in the application data layer) for this purpose. Once dual writes are stable and verified, the migration can progress to the next phase.
  **Note**: Mark a migration as ready for backfill phase using the `backfill_ready` in `config.toml`(_nexusd/src/migrations/config.toml_) by providing a comma separated list of migration ids.

- **Backfill**: In this phase, any missing or historical data in the new source is backfilled from the old source. This ensures that the new source is fully populated and consistent with the old source. The Migration Manager handles this phase automatically when the migration is progressed.

- **Cutover**: The application begins reading from the new source instead of the old source. For Redis, this often involves renaming keys (e.g., swapping the new key to the old key name). This phase ensures that the application is fully transitioned to the new source.

- **Cleanup**: The old source is no longer needed and can be safely cleaned up. This includes removing old keys in Redis or deleting data in Neo4j that is no longer required.
  Use the example at /examples/migration.rs as your guide.

### Adding a new migration

1. To create a new migration, use the migrations binary by running:

```bash
cargo run -p nexusd -- db migration new TagCountsReset
```

This will generate a new migration file in the `nexusd/src/migrations/migrations_list` directory.

2. Next, register your migration in the `import_migrations` function in `nexusd/src/migrations/mod.rs` file, which ensures it is included in the migration lifecycle.

3. Once registered, implement the required phases (dual_write, backfill, cutover, and cleanup) in the generated file `nexusd/src/migrations/migrations_list/tag_counts_reset_1739459180.rs`. Each phase serves a specific purpose in safely transitioning data between the old and new sources.

### Run the migration

After implementing your migration, run the migrations to execute pending migrations:

```bash
cargo run -p nexusd db migration run
```

The manager will automatically handle migrations in the appropriate order, progressing through phases as needed.

## 🧪 Running Tests

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

# nexus-watcher tests need the Postgres Connection URL as env variable, adjust it as needed
# export TEST_PUBKY_CONNECTION_STRING=postgres://test_user:test_pass@localhost:5432/postgres?pubky-test=true
cargo nextest run -p nexus-watcher --no-fail-fast
```

To test specific feature(s):

```bash
cargo nextest run -p nexus-watcher files::create --no-fail-fast
```

## 🚀 Benchmarking

If you want to see the performance of the server you can run the benchmarks

```bash
# cargo bench -p nexus-webapi [--bench <endpoint>]
cargo bench -p nexus-webapi
# or if you want specific endpoint
cargo bench -p nexus-webapi --bench user
```

## ⚠️ Troubleshooting

If tests or the development environment seem out of sync, follow the [Running Tests](#-running-tests) steps to reload the mock data.

## 🤝 Contributing

To contribute to Nexus, follow these steps:

1. **Fork the Repository** and create a feature branch
2. **Write Tests**: Ensure new features and changes are tested and benchmarked
3. **Submit a Pull Request** and provide a description of the changes

## 🌐 Useful Links

- **Swagger API**:
  - Staging: [https://nexus.staging.pubky.app/swagger-ui/](https://nexus.staging.pubky.app/swagger-ui/)
  - Production: [https://nexus.pubky.app/swagger-ui/](https://nexus.pubky.app/swagger-ui/)
