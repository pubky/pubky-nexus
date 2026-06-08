# Nexus and HS Decentralization — Configuration Reference

These are technical notes describing the configuration fields related to
decentralization: indexing third-party homeservers (HSs), resolving which HS
each user is hosted on, and the limits/timeouts/backoffs that govern this.

All fields below live under `[watcher]` (or `[watcher.retry]`) in
`nexus-common/default.config.toml`, and are defined on `WatcherConfig` in
`nexus-common/src/config/watcher.rs`.

---

## 1. Background

Originally the watcher pointed at one default homeserver and bulk-ingested all of
its events. With decentralization, Nexus still bulk-indexes that default HS but
*also* indexes other ("third-party") homeservers on a per-user basis, using the
homeserver's user-events endpoint. A separate task resolves each user's
currently-published HS from PKDNS/DHT and records it as a
`(:User)-[:HOSTED_BY]->(:Homeserver)` edge, so the indexer knows which users to
pull from which HS.

This runs as parallel watcher threads started in `NexusWatcher::start`
(`nexus-watcher/src/service/mod.rs`):
- `default-homeserver` (Section 2)
- `external-homeservers` (Section 3)
- `user-hs-resolver` (Section 4), and
- `retry-processor` (`[watcher.retry]`, Section 5, applies to all indexing)

The sections below group each config field under the thread it drives.

---

## 2. Default homeserver indexing

The baseline, pre-decentralization path. Driven by the `default-homeserver`
thread.

### `homeserver`
- **Type / default:** `PubkyId` / Synonym's HS key (see `default.config.toml`).
- **What it does:** The single default, prioritized homeserver. Its events are
  bulk-ingested, and it is explicitly *excluded* from the third-party
  (`external-homeservers`) list so it is never double-indexed
  (`runner/key_based.rs`, `hs_by_priority`).
- **Notes:** Changing this re-points the entire default-HS pipeline. The HS is
  persisted to the graph on startup (`Homeserver::persist_if_unknown`).

### `events_limit`
- **Type / default / max:** `u16` / `50` in the shipped config (code default
  `1000`) / **max `1000`** (`MAX_EVENTS_LIMIT`).
- **What it does:** Maximum number of events fetched **per run** from the default
  HS. Validated at deserialize time (`deserialize_events_limit`): `0` is
  rejected, and values above the max are rejected rather than clamped.
- **Tuning:** Higher → more throughput per tick but larger batches and longer
  per-run latency. Lower → smoother but slower to drain a backlog.

### `watcher_sleep`
- **Type / default:** `u64` milliseconds / `5000`.
- **What it does:** Sleep between full runs for **both** event-processing threads
  (default + external). It is the master tick for indexing.
- **Tuning:** Lower → fresher data, more load on HSs and DBs. Higher → less load,
  more lag between an event being published and indexed.

---

## 3. Third-party ("key-based") homeserver indexing

The core of decentralization. Driven by the `external-homeservers` thread, which
walks every monitored HS *except* the default and pulls events per user via the
HS user-events endpoint. Configured in
`KeyBasedEventProcessorRunner::from_config` (`runner/key_based.rs`).

### `monitored_homeservers_limit`
- **Type / default:** `usize` / `50` (`DEFAULT_MONITORED_HOMESERVERS_LIMIT`).
- **What it does:** Upper bound on how many homeservers are monitored.
- **Notes:** Setting it to `1` means **only the default HS is monitored** —
  effectively disabling decentralized indexing.
- **Tuning:** Each additional monitored HS adds HS requests (and, upstream, PKDNS
  resolutions) per tick. Raise deliberately as the network of indexed HSs grows.

### `key_based_events_limit`
- **Type / default / max:** `u16` / `50`
  (`DEFAULT_KEY_BASED_EVENTS_LIMIT`) / **max `100`**
  (`MAX_KEY_BASED_EVENTS_LIMIT`). Validated at deserialize time
  (`deserialize_key_based_events_limit`).
- **What it does:** Maximum events **per user, per run** when pulling from
  non-default HSs.
- **Why the ceiling is lower than `events_limit`:** this limit is *per user*, not
  *per HS*. A run may touch many users across many HSs, so the per-user batch is
  kept small to bound total work and per-HS request size.

### Offline-HS backoff: `initial_backoff_secs` / `max_backoff_secs`
- **Type / default:** `u64` seconds / `60` and `3600`
  (`DEFAULT_INITIAL_BACKOFF_SECS`, `DEFAULT_MAX_BACKOFF_SECS`).
- **What it does:** Per-HS exponential backoff for homeservers found to be
  **offline/unreachable** (`runner/key_based_hs_backoff.rs`, `HomeserverBackoff`,
  constructed at `runner/key_based.rs`). After a failure the HS is skipped for
  `initial_backoff_secs`; the skip interval doubles on each consecutive failure,
  capped at `max_backoff_secs`.
- **Constraint:** `initial_backoff_secs` must not exceed `max_backoff_secs`
  (`HomeserverBackoff::new`).
- **Tuning:** Larger initial/cap → fewer wasted requests to dead HSs, but slower
  to notice one coming back. Smaller → faster recovery, more retry traffic.

> ⚠️ **Do not confuse these with `[watcher.retry].initial_backoff_secs` /
> `max_backoff_secs`.** Same names, different mechanism — see Section 5.

### HS public-key blacklist *(planned — not yet merged)*
- **Status:** Tracked by PR #906; **not present** in `WatcherConfig` on this
  branch yet.
- **Intended use:** A config list of HS pubkys that must **never** be indexed,
  applied before any third-party HS is contacted. Document fully once merged.

---

## 4. User → homeserver resolution

Driven by the `user-hs-resolver` thread. For each user it resolves the currently
published HS from PKDNS/DHT and persists/refreshes the
`(:User)-[:HOSTED_BY]->(:Homeserver)` edge with a `resolved_at` timestamp
(`service/user_hs_resolver.rs`). This is what tells the third-party indexer which
users belong to which HS.

### `hs_resolver_sleep`
- **Type / default:** `u64` milliseconds / `10000` (`DEFAULT_HS_RESOLVER_SLEEP`).
- **What it does:** Sleep between runs of the resolver task. **Independent** of
  `watcher_sleep` — resolution and indexing tick on separate clocks.
- **Tuning:** Lower → mappings react faster to users migrating HSs, more PKDNS/DHT
  traffic. Higher → less traffic, slower to notice a user's HS change.

### `hs_resolver_ttl`
- **Type / default:** `u64` milliseconds / `3_600_000` (1 hour,
  `DEFAULT_HS_RESOLVER_TTL`).
- **What it does:** Minimum age before a user's HS mapping is considered stale and
  eligible for re-resolution. A user whose `HOSTED_BY.resolved_at` is newer than
  this TTL is **skipped** on a resolver run (`user_hs_resolver.rs`, `run`),
  preventing redundant PKDNS lookups.
- **Tuning:** Lower → mappings stay fresher at the cost of far more PKDNS lookups.
  Higher → cheaper, but Nexus may keep pulling a user's events from an HS they
  have already left for up to ~`hs_resolver_ttl`.

---

## 5. Event retry & backoff — `[watcher.retry]`

Cross-cutting: applies to **all** indexing (default and third-party), driven by
the `retry-processor` thread. Defined on `EventRetryConfig`
(`nexus-common/src/config/watcher.rs`). Backoff parameters are selected per error
via `EventRetryConfig::get_backoff_params` / `get_max_retries_for_err`: *transient*
errors and *missing-dependency* errors use separate limits.

| Field | Default | Role |
|---|---|---|
| `max_retries` | `10` | Transient-error retry limit before an event is dead-lettered. |
| `max_dependency_retries` | `50` | Retry limit for `MissingDependency` — safety net for HSs that disappear silently (content gone, no DEL event). Higher than `max_retries` because the dependency may still arrive. |
| `initial_backoff_secs` | `10` | Base for exponential backoff on **transient** retries (seconds). |
| `max_backoff_secs` | `3600` | Backoff ceiling for transient retries. |
| `initial_missing_dep_backoff_secs` | `60` | Base for `MissingDependency` polling backoff. |
| `max_missing_dep_backoff_secs` | `3600` | Backoff ceiling for `MissingDependency`. |

> ⚠️ **Two distinct "backoff" systems — do not conflate them:**
>
> - **`[watcher].initial_backoff_secs` / `max_backoff_secs`** (Section 3) — skips
>   an **entire homeserver** found to be **offline/unreachable**.
> - **`[watcher.retry].initial_backoff_secs` / `max_backoff_secs`** — retries an
>   **individual event** that hit a **transient processing error**.
>
> They share field names but operate at different granularities (HS vs. event)
> and on different triggers (unreachable vs. transient failure).

---

## 6. Testnet / general

Relevant only insofar as they switch the HS/relay target during local/dev runs.

| Field | Default | Role |
|---|---|---|
| `testnet` | `false` | Run against a testnet homeserver/relay instead of mainnet. |
| `testnet_host` | `"localhost"` | Host for the testnet HS/relay; change only if it runs on another machine (e.g. Docker setups). |

Out of scope for decentralization: `[api]`, `[stack]`, and the `moderation_*`
fields.

---

## 7. Quick reference

| Field | TOML path | Type | Default | Max | Thread / consumer |
|---|---|---|---|---|---|
| `homeserver` | `[watcher]` | `PubkyId` | Synonym HS | — | default-homeserver |
| `events_limit` | `[watcher]` | `u16` | `50` (code `1000`) | `1000` | default-homeserver |
| `watcher_sleep` | `[watcher]` | `u64` ms | `5000` | — | default + external (shared tick) |
| `monitored_homeservers_limit` | `[watcher]` | `usize` | `50` | — | external-homeservers |
| `key_based_events_limit` | `[watcher]` | `u16` | `50` | `100` | external-homeservers |
| `initial_backoff_secs` | `[watcher]` | `u64` s | `60` | — | external-homeservers (per-HS offline) |
| `max_backoff_secs` | `[watcher]` | `u64` s | `3600` | — | external-homeservers (per-HS offline) |
| *HS blacklist* | `[watcher]` | *(list)* | — | — | external-homeservers *(planned, PR #906)* |
| `hs_resolver_sleep` | `[watcher]` | `u64` ms | `10000` | — | user-hs-resolver (tick) |
| `hs_resolver_ttl` | `[watcher]` | `u64` ms | `3_600_000` | — | user-hs-resolver (staleness) |
| `max_retries` | `[watcher.retry]` | `u32` | `10` | — | retry-processor (transient) |
| `max_dependency_retries` | `[watcher.retry]` | `u32` | `50` | — | retry-processor (missing-dep) |
| `initial_backoff_secs` | `[watcher.retry]` | `u64` s | `10` | — | retry-processor (transient) |
| `max_backoff_secs` | `[watcher.retry]` | `u64` s | `3600` | — | retry-processor (transient) |
| `initial_missing_dep_backoff_secs` | `[watcher.retry]` | `u64` s | `60` | — | retry-processor (missing-dep) |
| `max_missing_dep_backoff_secs` | `[watcher.retry]` | `u64` s | `3600` | — | retry-processor (missing-dep) |
| `testnet` | `[watcher]` | `bool` | `false` | — | startup |
| `testnet_host` | `[watcher]` | `String` | `"localhost"` | — | startup |
