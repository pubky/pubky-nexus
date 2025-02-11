![Integration Tests](https://github.com/pubky/pubky-nexus/actions/workflows/test.yml/badge.svg?branch=main)

# Pubky Nexus

Pubky Nexus is the central bridge connecting Pubky homeservers with Pubky-App’s social clients. By aggregating events from homeservers into a rich social graph, Nexus transforms decentralized interactions into a high-performance, fully featured social-media-like API. It's designed to support Social-Semantic-Graph (SSG) inference, and more.

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

- **service.rs**: The REST API server for handling client requests, querying databases, and returning responses to the Pubky-App frontend.
- **watcher.rs**: The event aggregator that listens to homeserver events, translating them into social graph updates within the Nexus databases.
- **lib.rs**: A library crate containing common functionalities shared by `service` and `watcher`, including database connectors, models, and queries.

### Data Flow

![pubky-nexus-arch](docs/images/pubky-nexus-arch.png)

1. **Event Ingestion**: The watcher ingests events from Pubky homeservers and indexes them into our social graph.
2. **Indexing and Caching**: Relationships, common queries, and graph data are cached in Redis for high-speed access. Complex queries use Neo4j.
3. **API Responses**: The service server reads from these indexes to respond to client queries efficiently.

![pubky-nexus-graph](docs/images/pubky-nexus-graph.png)

Nexus graph schema.

## ⚙️ Setting Up the Development Environment

To get started with Nexus, first set up the required databases: Neo4j and Redis.

### 1. Configure Databases

1. Clone the repository and navigate to the project directory.
2. Copy the environment template and set up the Docker environment:

   ```bash
   cd docker
   cp .env-sample .env
   docker-compose up -d
   ```

3. Optionally, populate the Neo4j database with initial mock data. Follow [Running Tests](#running-tests) section about setting up mock data.  

4. Run the Nexus service:

   ```bash
   cargo run
   ```

5. Run the Watcher service:

   ```bash
   cargo run --bin watcher
   ```

6. **Access Redis and Neo4j UIs**:
   - Redis UI: [http://localhost:8001/redis-stack/browser](http://localhost:8001/redis-stack/browser)
   - Neo4J UI: [http://localhost:7474/browser/](http://localhost:7474/browser/)

## 🚀 Contributing

To contribute to Nexus, follow these steps:

1. **Fork the Repository** and create a feature branch.
2. **Write Tests**: Ensure new features and changes are tested and benchmarked.
3. **Submit a Pull Request** and provide a description of the changes.

### Data Migrations

The Migration Manager is a purpose-built tool designed to simplify and standardize the process of performing data migrations in our backend system. It ensures a smooth transition during breaking changes to our data sources, such as Neo4j and Redis, by coordinating phased migrations with minimal disruption to the application. The manager tracks the status of each migration in the database, automates phase progression where possible, and provides a clear structure for developers to implement and manage migrations. This approach reduces the risk of data inconsistencies, ensures reliability during deployments, and keeps migration-related code isolated and easy to find.

#### Understanding Migration Phases

The Migration Manager uses a phased approach to handle data migrations safely and systematically. Each phase serves a distinct purpose in transitioning data from the old source to the new source, ensuring consistency and minimal disruption. Here's an overview of the phases:

- **Dual Write**: During this phase, all writes to the old source are mirrored to the new source. This ensures that both sources remain synchronized during normal application operations. Developers invoke MigrationManager::dual_write in the application logic (preferrably in the application data layer) for this purpose. Once dual writes are stable and verified, the migration can progress to the next phase.
  **Note**: Mark a migration as ready for backfill phase using the `MIGRATIONS_BACKFILL_READY` env var by providing a comma separated list of migration ids.

- **Backfill**: In this phase, any missing or historical data in the new source is backfilled from the old source. This ensures that the new source is fully populated and consistent with the old source. The Migration Manager handles this phase automatically when the migration is progressed.

- **Cutover**: The application begins reading from the new source instead of the old source. For Redis, this often involves renaming keys (e.g., swapping the new key to the old key name). This phase ensures that the application is fully transitioned to the new source.

- **Cleanup**: The old source is no longer needed and can be safely cleaned up. This includes removing old keys in Redis or deleting data in Neo4j that is no longer required.
  Use the example at /examples/migration.rs as your guide.

#### Adding a new migration

To create a new migration, use the migrations binary by running:

```
cargo run --bin migrations new MigrationNAME
```

This will generate a new migration file in the `src/db/migrations/migrations_list` directory.  
Next, register your migration in the `get_migration_manager` function in `src/db/migrations/mod.rs` file, which ensures it is included in the migration lifecycle. Once registered, implement the required phases (dual_write, backfill, cutover, and cleanup) in the generated file. Each phase serves a specific purpose in safely transitioning data between the old and new sources. After implementing your migration, run the migrations binary to execute pending migrations:

```
cargo run --bin migrations run
```

The manager will automatically handle migrations in the appropriate order, progressing through phases as needed.

### Running Tests

Running tests requires setting up mock data into Neo4j and Redis.

Use the `mockdb` binary to load the mock data.

```bash
cargo run --bin mockdb [database]
```

`database` is optional and can be either `graph` or `redis`. Not providing any value will sync all the databases.

> If the Redis cache is empty, the nexus-service will handle it automatically. If not follow the steps of warning section

You can optionally pass the `GRAPH_CONTAINER_NAME` env var if your neo4j container in docker has a different name. Defaults to `neo4j`. For example:

```bash
GRAPH_CONTAINER_NAME=nexus-neo4j cargo run --bin mockdb
```

Then to run all tests:

```bash
cargo test // cargo nextest run
```

To test specific modules or features:

```bash
cargo test watcher:users
cargo test test_homeserver_user_event
```

**Benchmarking**:

```bash
cargo bench --bench user get_user_view_by_id
```

## ⚠️ Troubleshooting

If tests or the development environment seem out of sync, follow the [Running Tests](#running-tests) steps to reload the mock data.

## 🌐 Useful Links

- **Swagger API**:
  - Staging: [https://nexus.staging.pubky.app/swagger-ui/](https://nexus.staging.pubky.app/swagger-ui/)
  - Production: [https://nexus.pubky.app/swagger-ui/](https://nexus.pubky.app/swagger-ui/)
- **Local Redis Insight**: [http://localhost:8001/redis-stack/browser](http://localhost:8001/redis-stack/browser)
- **Local Neo4J Browser**: [http://localhost:7474/browser/](http://localhost:7474/browser/)
