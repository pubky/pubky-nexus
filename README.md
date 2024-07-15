# Pubky-App Backend
Pubky-app bakend in rust. Listens to the homeserver(s), indexes events as a social media and serves them to the pubky-app clients.

# 💻 Development Roadmap
### [Pubky Backend Development Roadmap](https://github.com/pubky/pubky-app-backend/issues/1)

# 🏗️ Objective for Alpha v0.1.0 Milestone
Reach feature parity with `skunk-work` indexer improving on the following:
1. High performance.
2. Clear vision forward.
3. Free of bugs.
4. Cleaner dev experience.

# 🏠Architecture
![image](https://github.com/user-attachments/assets/e516ceff-d28f-4d71-9123-96eb1725cd73)

1. The watcher does effectively work as an aggregator/translator.
2. The service reads from indexes or queries the graph and serves the pubky-app clients.
3. The social graph DB (Neo4J) is intended for holding a complete view of the network. It should be queried as little as possible but we can abuse it at the beginning in order to complete features faster.
4. As long as it is possible and not too troublesome, most relationships, query results and cache should be indexed by `key: value` and retrieved from Redis. We should take inspiration on current use of LMDB in `skunk-works`.
