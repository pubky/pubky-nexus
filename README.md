![Integration Tests](https://github.com/pubky/pubky-nexus/actions/workflows/test.yml/badge.svg?branch=main)

# Pubky-Nexus

The Nexus between Pubky homeservers and Pubky-App social frontend. Pubky Nexus constructs a social graph out of all of the events on pubky-core homeservers and exposes a social-media-like API capable of powerful Web-of-Trust inference.

## 💻 Development Roadmap

### [Pubky Backend Development Roadmap](https://github.com/pubky/pubky-app-backend/issues/1)

## 🏗️ Objective for Alpha v0.1.0 Milestone

Reach feature parity with `skunk-work` indexer improving on the following:

1. High performance: no inefficient lookups, maximum normalization, maximum atomic indexing, full async, full multi-thread, rust performance.
2. Clear vision forward: esimplify the implementation of exciting future features: WoT, graph queries, etc.
3. Free of bugs: hopefully.
4. Cleaner dev experience.
5. Moden stack.
6. Excellent observability (browse over our indexes with [redis-insight](https://redis.io/insight/) or graph with [neo4j-browser](https://browser.neo4j.io/))

## 🏠Architecture

![pubky-nexus-arch](docs/images/pubky-nexus-arch.png)

- **service.rs**: binary that serves REST request to the pubky-app clients reading from our DBs.
- **watcher.rs**: binary that subscribes to homeservers and populate our DBs
- **lib.rs**: library crate with all of the common functionalities (connector, models, queries) needed for `watcher` and `service`

1. The watcher does effectively work as an aggregator (a translator from Homeserver events to a social network graph).
2. The service reads from the indexes and performs queries to the graph in order to serve responses to the pubky-app clients.
3. As long as it is possible and not too troublesome, most relationships, query results and cache should be indexed by `key: value` and retrieved from Redis. We should take inspiration on current use of LMDB in `skunk-works` (a lot of things can be done using plain `key: value` but some are too troublesome to implement: then we query our graph directly)
4. The social graph DB (Neo4J) is intended for holding a complete view of the network. It should be queried as little as possible but we can abuse it at the beginning in order to complete features faster.

![pubky-nexus-graph](docs/images/pubky-nexus-graph.png)

## ⚙️ Preparing the Environment

Before running the project, several configurations must be set up. Let’s start by configuring the databases:

```bash
cd docker
# Create a new `.env` file from the `.env-sample` template
cp .env-sample .env
# Run the databases (Neo4j and Redis databases will spin up empty)
docker-compose up -d
# Populate the graph database with initial data
docker exec neo4j bash /db-seed/run-queries.sh
```

Once the `Neo4j` graph database is seeded with data, the next step is to populate the `Redis` database by running the _nexus-service_:

```bash
# Navigate back to the project root if you're still in the docker folder
cd ..
# Copy the .env example to the active .env file
cp .env-sample .env
# Set the REINDEX environment variable to true for the reindexing process
export REINDEX=true
# Start the reindexing process
cargo run
# Once the process is finished, you can unset the REINDEX environment variable
unset REINDEX
```

## 👨‍💻 Quick Development Setup

To enable auto-rebuilding and testing while developing within the `/service`:

```bash
# Install `cargo-watch` to monitor changes and auto-rebuild on save
cargo install cargo-watch
# Ensure the environment variables are set. You might have already done this in the previous step:
cp .env-sample .env
# Run the service and tests in separate terminals:

# Terminal 1: Start the service with auto-reload on changes:
cargo watch -q -c -w src/ -x "run --bin service"
# The service will be available at localhost:8080/v0/info on your browser

# Terminal 2: Run tests (note that for tests to pass, the Neo4j instance must have example data)
# Ensure you've followed the steps above to set up Neo4j with the example dataset
cargo watch -q -c -w tests/ -x "test -- --nocapture"

# Run benchmarks (e.g., get user by ID benchmark)
cargo bench --bench user get_user_view_by_id
```

## Developing the homeserver watcher

Running the `/tests/` that require the homeserver does not require running a homeserver. However, running the playground or the `watcher.rs` binary does. This is how you can run a pubky homeserver locally in testnet mode.

We are using `pubky` repo as a git submodule of `pubky-nexus`, given that `pubky` is still a private repository and the crates for the client and homeserver are not yet published.

```bash
git submodule init
git submodule update --init --recursive
cd pubky/pubky-homeserver
cargo run -- --testnet
```

Take a look at the logs for

1) `testnet.bootstrap=["127.0.0.1:6881"]`
2) Your homeserver listening url `http://localhost:15411` and
3) the pubky URI `pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo` and make sure your `.env` has the correct settings

## ⚠️ Warning

There are scenarios where the integration tests might fail. This typically occurs when the database data is out of sync with the current integration tests. To resolve this, you need to reset the Neo4j graph database and re-seed it with the correct data. Follow these steps:

1. Open the Neo4j browser interface at `http://localhost:7474/browser/` and log in using the credentials found in the `/docker/.env` file.
2. Run the following Cypher query to remove all nodes and relationships in the database:

```cypher
MATCH (n)
DETACH DELETE n;
```

3. Once the graph is cleared, re-populate the database with the correct dataset:

```bash
docker exec neo4j bash /db-seed/run-queries.sh
# Reindex environment variable TRUE, check above
cargo run
# and unset the reindex variable
```

## Useful links

- Swagger UI: http://localhost:8080/swagger-ui/
- Redis: http://localhost:8001/redis-stack/browser
- Neo4J: http://localhost:7474/browser/