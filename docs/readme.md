# Documentation

The [README](../README.md) says what Nexus is, how to reach its API and what a version promises; the API itself is described by its OpenAPI documents. This folder holds the rest: why Nexus is built the way it is, and how to run and release it.

| Folder | Holds | Changes |
|---|---|---|
| [`adr/`](adr) | Architecture decision records: why | Never, once accepted. A new record replaces it. |
| [`guides/`](guides) | References and procedures: how | With the code, in the same pull request. |
| [`images/`](images), [`files/`](files) | Diagrams | When the architecture or the graph does. |

## Decisions (`adr/`)

| Record | Decision | Status |
|---|---|---|
| [0003](adr/0003-per-user-migration-to-social-v1.md) | Users move from `pubky.app` to `social/v1` one at a time, each read through one data model, the one their epoch names | Proposed |

An ADR records one decision that shapes the code and that someone will later want to question: the context, what was chosen, the consequences and the alternatives rejected. Write one when a pull request makes such a decision. A fix or a refactor that changes no decision needs none.

- **Name:** `NNNN-short-title.md`, numbered after the highest record here; the next one is `0004`.
- **Sections:** Status, Context, Decision, Consequences, Alternatives Considered, as in 0003.
- **Status:** `Proposed` while the pull request is open, `Accepted` once it merges, `Superseded by NNNN` when a later record replaces it. Each status carries its date.
- **Once accepted,** the decision is not rewritten. A change of mind is a new record that supersedes it, and the old one stays for the history.

## Guides (`guides/`)

| Guide | For | Covers |
|---|---|---|
| [development.md](guides/development.md) | Contributors | The local databases, running Nexus, the mock data, the tests and the benchmarks |
| [homeservers.md](guides/homeservers.md) | Operators | How the watcher indexes the primary homeserver and externally-hosted users, resolves each user's homeserver and retries events, with the config fields, metrics and alerts of each |
| [jobs.md](guides/jobs.md) | Operators | The jobs `nexusd` runs on a schedule or on demand, and how to configure them |
| [migrations.md](guides/migrations.md) | Operators, contributors | Running the pending migrations before a release, how the phases advance, and writing a new migration |
| [observability.md](guides/observability.md) | Operators | OTLP telemetry, the local Grafana stack, the alerting rules, SigNoz |
| [releasing.md](guides/releasing.md) | Maintainers | Choosing a version, the release steps, the checks after the tag, and what to do with a bad release |

A guide describes the code as it is, so a pull request that changes the behaviour or a config field a guide covers updates the guide too. Name a guide after what it covers, not after the project that introduced it.

## Diagrams (`images/`, `files/`)

- [pubky-nexus-arch.png](images/pubky-nexus-arch.png): the architecture, from the homeservers through the watcher, Neo4j and Redis to the API. Shown in the README.
- [pubky-nexus-graph.png](images/pubky-nexus-graph.png): the graph schema. Shown in the README.
- [pubky-nexus.tldr](files/pubky-nexus.tldr): the editable source of the graph schema diagram. Import it into [tldraw](https://www.tldraw.com/) to view or change it, then export the changed diagram as a PNG into `images/`.
