# Nexus and HS Decentralization — Configuration Reference

These are technical notes describing configuration fields related to Decentralization.

---

## 1. Background

Originally the watcher pointed at one primary HS and bulk-ingested all of
its events. With decentralization, Nexus still bulk-indexes the primary HS but
*also* indexes users hosted on other ("third-party") HSs on a per-user
basis, using each HS's user-events endpoint. A separate task resolves each user's
currently-published HS from PKDNS/DHT and records it as a
`(:User)-[:HOSTED_BY]->(:Homeserver)` edge, so the indexer knows which users to
pull from which HS.

These run as parallel tasks started in `NexusWatcher::start`, each driving one
runner; the sections below group each config field under the runner it drives:

- [Section 2: Bulk indexing of primary HS](#2-indexing-the-primary-hs-bulk)
- [Section 3: Key-based indexing of externally-hosted users](#3-indexing-externally-hosted-users-key-based)
- [Section 4: User → HS resolution](#4-user--hs-resolution)
- [Section 5: Event retry & backoff](#5-event-retry--backoff--watcherretry)
- [Section 6: Quick reference](#6-quick-reference)

---

## 2. Indexing the primary HS (bulk)

The baseline, pre-decentralization path: the primary HS is indexed in *bulk* — all
of its events are pulled from the HS `/events` endpoint. Driven by `HsEventProcessorRunner`.

The primary HS is trusted, which implies a different event validation model.

### `homeserver`

> The single primary, prioritized HS. Its events are bulk-ingested.

It is explicitly *excluded* from the third-party (`KeyBasedEventProcessorRunner`)
list so it is never double-indexed (`hs_by_priority`). Changing this re-points
the entire primary-HS pipeline; the HS is persisted to the graph on startup
(`Homeserver::persist_if_unknown`).

### `events_limit`

> Maximum number of events fetched **per run** from the primary HS.

Validated at deserialize time (`deserialize_events_limit`): `0` is rejected, and
values above the max are rejected rather than clamped.

*Tuning:* higher → more throughput per tick but larger batches and longer
per-run latency. Lower → smoother but slower to drain a backlog.

### `primary_hs_monitoring_interval_ms`

> Scheduling interval[^1] for triggering runs of the **primary-HS** indexing runner
> (`HsEventProcessorRunner`).

*Tuning:* lower → fresher data, more load on HSs and DBs. Higher → less load,
more lag between an event being published and indexed.

---

## 3. Indexing externally-hosted users (key-based)

The core of decentralization. Driven by `KeyBasedEventProcessorRunner`, which
indexes users hosted on **third-party** HSs — any HS other than the primary
(also called "external" or "secondary"). For every monitored HS *except* the
primary, it pulls each hosted user's events per user from the HS `/events-stream`
endpoint (hence "key-based" — keyed on each user's pubky). Configured in
`KeyBasedEventProcessorRunner::from_config`.

### `monitored_homeservers_limit`

> Bounds the number of **third-party** HSs monitored.

`0` disables third-party-HS indexing; `1` monitors one third-party HS.

*Tuning:* each additional monitored HS adds HS requests (and, upstream, PKDNS
resolutions) per tick. Raise deliberately as the network of indexed HSs grows.

### `external_hs_monitoring_interval_ms`

> Scheduling interval[^1] for this `KeyBasedEventProcessorRunner` (the external-HS
> monitoring task). Independent of `primary_hs_monitoring_interval_ms` so the two
> cadences can be tuned separately.

*Tuning:* lower → fresher data from external HSs, more load on them and the DBs.
Higher → less load, more lag. External-HS runs typically touch many users across
many HSs, so this is often set larger than `primary_hs_monitoring_interval_ms`.

### `key_based_events_limit`

> Maximum events **per user, per run** when pulling from third-party HSs.

Validated at deserialize time (`deserialize_key_based_events_limit`).

*Why the ceiling is lower than `events_limit`:* this limit is *per user*, not
*per HS*. A run may touch many users across many HSs, so the per-user batch is
kept small to bound total work and per-HS request size.

### `initial_backoff_secs` / `max_backoff_secs` — offline-HS backoff

> Per-HS exponential backoff for third-party HSs found to be **offline/unreachable**
> (`HomeserverBackoff`). After a failure the HS is skipped for `initial_backoff_secs`;
> the skip interval doubles on each consecutive failure, capped at `max_backoff_secs`.

*Constraint:* `initial_backoff_secs` must not exceed `max_backoff_secs`
(`HomeserverBackoff::new`).

*Tuning:* larger initial/cap → fewer wasted requests to dead HSs, but slower to
notice one coming back. Smaller → faster recovery, more retry traffic.

> ⚠️ **Do not confuse these with `[watcher.retry].initial_backoff_secs` /
> `max_backoff_secs`.** Same names, different mechanism — see
> [Section 5](#5-event-retry--backoff--watcherretry).

### `external_hs_pk_blacklist` — HS public-key blacklist

> Configured in `[stack.net]`. List of third-party HS PKs from which new events
> are not being indexed, for as long as they are on this list. Consulted when
> indexing third-party HSs, and also checked when ingesting new users (e.g. via
> the Nexus REST API).

Each entry is parsed as a `PubkyId` at deserialize time, so an invalid pubky in
the list fails config load rather than being silently ignored
(`test_external_hs_pk_blacklist_rejects_invalid_pk`).

*Effect on existing data:* existing events from users pointing to a listed HS are
not affected. New users pointing to a listed HS will not be ingested.
Already-ingested users who now point to a blacklisted HS keep their old data;
only new events from the blacklisted HS are not indexed.

*Effect on dependencies:* events depending on a not-yet-ingested user hosted by a
blacklisted HS (a follow, a tag, a reply or repost referencing their posts) are
dropped rather than queued for retry, since the dependency cannot be ingested
while blacklisted. Removing the HS from the list later does not recover these
dropped events. Posts that merely mention such a user are still indexed; only the
mention relationship is not materialized.

---

## 4. User → HS resolution

Driven by `UserHsResolverRunner`. For each user it resolves the currently
published HS from PKDNS/DHT and persists/refreshes the
`(:User)-[:HOSTED_BY]->(:Homeserver)` edge with a `resolved_at` timestamp.
This is what tells the externally-hosted-user indexer
([Section 3](#3-indexing-externally-hosted-users-key-based)) which users
belong to which HS.

### `hs_resolver_interval_ms`

> Scheduling interval[^1] for triggering runs of the resolver task.

**Independent** of `primary_hs_monitoring_interval_ms` and
`external_hs_monitoring_interval_ms` — resolution and indexing tick on separate clocks.

*Tuning:* lower → mappings react faster to users migrating HSs, more PKDNS/DHT
traffic. Higher → less traffic, slower to notice a user's HS change.

### `hs_resolver_ttl`

> Minimum age before a user's HS mapping is considered stale and eligible for
> re-resolution.

A user whose `HOSTED_BY.resolved_at` is newer than this TTL is **skipped** on a
resolver run, preventing redundant PKDNS lookups.

*Tuning:* lower → mappings stay fresher at the cost of far more PKDNS lookups.
Higher → cheaper, but Nexus may keep pulling a user's events from an HS they have
already left for up to ~`hs_resolver_ttl`.

---

## 5. Event retry & backoff — `[watcher.retry]`

Cross-cutting: applies to **all** indexing, driven by `RetryProcessor`. Backoff
parameters and retry limits are selected per error via
`EventRetryConfig::get_backoff_params` / `get_max_retries_for_err`: *transient*
errors and *missing-dependency* errors use separate values.

### `max_retries` / `max_dependency_retries` — retry limits

> Maximum retry attempts before an event is dead-lettered. `max_retries` applies to
> **transient** errors; `max_dependency_retries` applies to `MissingDependency`.

`max_dependency_retries` is higher than `max_retries` because it is a safety net
for HSs that disappear silently (content gone, no DEL event) — the missing
dependency may still arrive, so it is worth polling for longer.

### `initial_backoff_secs` / `max_backoff_secs` — transient-error backoff

> Exponential backoff for re-trying an **individual event** that hit a **transient**
> processing error: `initial_backoff_secs` is the base delay, doubling on each
> attempt, capped at `max_backoff_secs`.

### `initial_missing_dep_backoff_secs` / `max_missing_dep_backoff_secs` — missing-dependency backoff

> Exponential backoff for polling a `MissingDependency`: `initial_missing_dep_backoff_secs`
> is the base delay, doubling on each attempt, capped at `max_missing_dep_backoff_secs`.

Kept separate from the transient-error backoff above because a missing dependency
is waited-on rather than retried-against — it starts slower (`60` s vs `10` s) to
avoid hammering an HS for content that may not exist yet.

> ⚠️ **Two distinct "backoff" systems — do not conflate them:**
>
> - **`[watcher].initial_backoff_secs` / `max_backoff_secs`**
>   ([Section 3](#3-indexing-externally-hosted-users-key-based)) — skips
>   an **entire HS** found to be **offline/unreachable**.
> - **`[watcher.retry].initial_backoff_secs` / `max_backoff_secs`** — retries an
>   **individual event** that hit a **transient processing error**.
>
> They share field names but operate at different granularities (HS vs. event)
> and on different triggers (unreachable vs. transient failure).

---

## 6. Quick reference

| Field | TOML path | Type | Default |
|---|---|---|---|
| `homeserver` | `[watcher]` | `PubkyId` | Synonym HS |
| `events_limit` | `[watcher]` | `u16` | `50` (max `1000`; code default `1000`) |
| `primary_hs_monitoring_interval_ms` | `[watcher]` | `u64` ms | `5000` |
| `external_hs_monitoring_interval_ms` | `[watcher]` | `u64` ms | `5000` |
| `monitored_homeservers_limit` | `[watcher]` | `usize` | `50` |
| `key_based_events_limit` | `[watcher]` | `u16` | `50` (max `100`) |
| `initial_backoff_secs` | `[watcher]` | `u64` s | `60` |
| `max_backoff_secs` | `[watcher]` | `u64` s | `3600` |
| `hs_resolver_interval_ms` | `[watcher]` | `u64` ms | `10000` |
| `hs_resolver_ttl` | `[watcher]` | `u64` ms | `3_600_000` |
| `max_retries` | `[watcher.retry]` | `u32` | `10` |
| `max_dependency_retries` | `[watcher.retry]` | `u32` | `50` |
| `initial_backoff_secs` | `[watcher.retry]` | `u64` s | `10` |
| `max_backoff_secs` | `[watcher.retry]` | `u64` s | `3600` |
| `initial_missing_dep_backoff_secs` | `[watcher.retry]` | `u64` s | `60` |
| `max_missing_dep_backoff_secs` | `[watcher.retry]` | `u64` s | `3600` |
| `external_hs_pk_blacklist` | `[stack.net]` | `Vec<PubkyId>` | `[]` |


[^1]: This is the *interval* at which runs are *triggered*, not a sleep duration
that starts only after a run finishes. If a run takes longer than the interval,
the next run starts right after it returns.
