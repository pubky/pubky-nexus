# Pubky-App Backend

Pubky-app bakend in rust. Listens to the homeserver(s), indexes events as a social media and serves them to the pubky-app clients.

## 💻 Development Roadmap

### [Pubky Backend Development Roadmap](https://github.com/pubky/pubky-app-backend/issues/1)

## 🏗️ Objective for Alpha v0.1.0 Milestone

Reach feature parity with `skunk-work` indexer improving on the following:

1. High performance: no inefficient lookups, rust performance, modern stack.
2. Clear vision forward: easy implementation of exciting features: WoT, graph queries, etc.
3. Free of bugs: hopefully.
4. Cleaner dev experience.

## 🏠Architecture

![image](https://github.com/user-attachments/assets/e516ceff-d28f-4d71-9123-96eb1725cd73)

- **/service** is a binary crate that serves REST request to the pubky-app clients reading from our DBs.
- **/watcher** is a binary crate that will subscribe to homeservers and populate our DBs
- **/common** is a library crate with all of the common functionalities needed for the `watcher` and the `service`

1. The watcher does effectively work as an aggregator (a translator from Homeserver events to a social network graph).
2. The service reads from the indexes and performs queries to the graph in order to serve responses to the pubky-app clients.
3. The social graph DB (Neo4J) is intended for holding a complete view of the network. It should be queried as little as possible but we can abuse it at the beginning in order to complete features faster.
4. As long as it is possible and not too troublesome, most relationships, query results and cache should be indexed by `key: value` and retrieved from Redis. We should take inspiration on current use of LMDB in `skunk-works` (a lot of things can be done using plain `key: value` but some are too troublesome to implement: then we query our graph directly)

## 👨‍💻 Quick Dev

For auto re-build on save and testing while developing `/service` :

```bash
cargo install cargo-watch
cd service

# Ideally in two terminals.
# On terminal 1 run:
cargo watch -q -c -w src/ -x run
# You can check the running service on your browser on localhost:8080/hello

# On terminal 2 run (for tests to work you need a working /neo4j-example instance with example dataset)
cargo watch -q -c -w tests/ -x "test -- --nocapture"

```
