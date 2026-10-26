# Trying to break a homeserver migration

This suite exists to be attacked. A migration is a user moving their data
from homeserver A to homeserver B and publishing a `_pubky` record naming B.
Nexus has to end up indexing the user from B with nothing lost, nothing
counted twice and nobody notified twice, whenever it notices the switch.
Your job is to find an order of events where that does not hold.

## What a migration looks like

- A user signs up on B. The SDK publishes the record naming B at once, so the
  switch happens before any copy. Then the client copies the public tree,
  file by file, and may delete everything on A.
- Nexus learns about the switch in one of two ways: the app calls
  `PUT /v0/recheck/{user_id}` (a fresh network lookup, the record is
  followed), or the periodic resolver sweep visits the user with stale
  mapping disabled. Until then it keeps reading A.
- After the switch the user is read on B from their per-user cursor there,
  0 if they were never on B, and every event's content is fetched from the
  homeserver whose stream delivered it.
- If B is the primary homeserver, its stream is read in bulk with one global
  cursor and events of users bound elsewhere are skipped for good. That is
  where the known gap is.

## Running

Each test starts its own testnet with three homeservers (`primary`, `hs_a`,
`hs_b`) and needs its own process, because nexus's Pubky client is a
process-wide singleton. Use nextest, never `cargo test` for the whole suite:

```
cargo nextest run -p nexus-watcher --test migration
cargo nextest run -p nexus-watcher --test migration -E 'test(hazards)' --run-ignored all
```

Run it against a private stack rather than the shared databases, so leftovers
from other runs cannot fail an assertion. `stack.sh` in this directory starts
a Redis, a Neo4j and a Postgres in their own network namespace and runs
commands inside it (needs docker, sudo and nsenter):

```
nexus-watcher/tests/migration/stack.sh up
nexus-watcher/tests/migration/stack.sh run cargo nextest run --offline -p nexus-watcher --test migration
nexus-watcher/tests/migration/stack.sh down
```

Pass `--offline` to cargo inside the namespace, build first if you can, and
`down` the stack when done: every test leaves three `pubky_test_*` Postgres
databases behind. If you must use the shared databases, rerun a failure on
`main` before blaming your change.

## The harness

`MigrationTest` (`harness.rs`) gives you both sides.

- Users: `device()` is one SDK client with its own pkarr cache, so an old
  device keeps resolving the old homeserver for an hour, as in production.
  `signup`, `session`, `post`, `follow`, `tag`, `write_profile`,
  `delete_profile`, `public_tree` (read through a device), `write_tree` (the
  copy), `delete_tree` (the wipe), `switch_record`,
  `publish_without_homeserver`. Two fixtures bundle the fixed parts of a
  migration: `resident` (sign up with posts, ingested and read once) and
  `signup_and_copy` (sign up elsewhere and copy the tree). Spell the steps
  out whenever your scenario puts something between them.
- Nexus, one step at a time: `ingest` (how nexus first meets an unknown user),
  `recheck` (the route), `resolve` (one sweep, stale mapping disabled, only
  this user looked up, with a fresh lookup), `resolve_cached(user, client)`
  (the same sweep, looking up cache-first through `client` as production does;
  a second client stands for the watcher process when the recheck runs in the
  API), `index_external(hs)` (one per-user read), `index_external_paused(hs,
  user)` (a read that has listed its users and waits before fetching `user`
  until `resume()`), `index_primary` (one bulk poll), `catch_up` (a per-user
  read of the primary, which production does not do yet), `retry` (one pass
  of the retry processor, every entry due).
- The retry queue is in memory, one per test.
- What nexus holds: `binding`, `bound_to`, `bound_name`, `cursor`,
  `bulk_cursor`, `live_posts`, `profile_state`, `notifications`.

Assert outcomes, not mechanisms: what is indexed, which homeserver the user is
bound to, cursors, notification kinds. Never the resolver's internal flags.
Notifications with the same body share one entry in Redis and the entry takes
the latest time, so a notification issued twice is one entry with a newer
time: compare `notification_times`, not `notifications`, when the question is
whether something was notified again.

## Where to push

Vary the moment nexus notices relative to everything else, and run every
scenario in both directions, external to external and to or from the primary.

- Notice before the copy starts, in the middle of it, after it.
- Mirror to B while the record still names A, then switch, then wipe A.
- Wipe A before nexus notices; wipe only part of it.
- Flap A, B, A with writes on each in between; return after a long time.
- Edit the same post on A and B with different content; delete on one side.
- Write from a device that still resolves the old homeserver.
- Records that name a dead key, a homeserver without an account, a
  blacklisted homeserver, the primary without an account, no homeserver at
  all; a record older than the one applied.
- A user nexus knows only as a stub; a tombstoned user; a user with
  thousands of events; replies and tags from others on the moving posts.
- Two things at once: a recheck during a per-user read, a recheck during the
  sweep, two rechecks together, a switch while the retry queue still holds
  the user's events from A.
- Cost: what one call to the recheck makes nexus do on the network and in
  the graph, and how many such calls a stranger can make.

Broken means: an event the user wrote is never indexed; a post, count, edge
or notification exists twice; the user is bound to a homeserver their record
does not name; the user is indexed nowhere while their record names a live
homeserver; an interactor is notified of a change the user did not make.

## What the testnet cannot show

- Replays of old records: the testnet DHT nodes all hold the newest packet
  and reject an older one, so a replay cannot be served. On mainnet a node
  or relay can still answer with a packet older than the one nexus applied,
  so reason about those cases instead of running them.
- A homeserver reachable only through an ICANN domain: testnet homeservers
  publish a direct endpoint too.
- Rebuilt homeservers whose event ids restart.
- Real latency: lookups take milliseconds here, seconds on mainnet.

## When you find something

Keep the scenario as a test. If it fails on purpose, mark it `#[ignore]` with
a reason that says what has to exist for it to pass, as `hazards.rs` does for
the primary gap. Put the finding in the test's doc comment: the order of
events, what nexus does, what it should do, and whether you demonstrated it or
reasoned it. The commit message carries the summary.
