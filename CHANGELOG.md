# Changelog

Each version of Nexus has a section below, newest first. What a version number promises is in the [README](README.md), under Versioning; how a release is cut is in [docs/guides/releasing.md](docs/guides/releasing.md).

## 1.0.0

The first stable release. Nexus 1.x serves the `/v1` API and reads the 1.x data model of [`pubky-social-specs`](https://github.com/pubky/pubky-social-specs). The version of Nexus and that of the specs crate move independently from here.

### Upgrade

- Before starting 1.0.0, run the pending migrations with `nexusd db migration run`. `nexusd db migration check` must then exit 0.
- `/v0` is deprecated. It keeps working, frozen, until a later release announces its removal.
- The Rust crates are no longer published to crates.io. 0.4.1 was the last version published there.

### What Nexus 1.0.0 is

**API**

- `/v1` is the current API, read by the `social/v1` clients. `/v0` serves the clients that have not moved yet.
- The OpenAPI documents are `/api-docs/v0/openapi.json`, `/api-docs/v1/openapi.json` and `/api-docs/static/openapi.json` for the `/static` file routes. The Swagger UI is at `/swagger-ui`.
- `/v0/info` and `/v1/info` report the running version.

**Ingestion**

- The watcher follows the configured primary homeserver through its event feed, and other homeservers through a per-user event stream. It picks those homeservers by the trust of the users they host, up to `monitored_homeservers_limit`.
- A user's homeserver is resolved through PKARR. A user seen for the first time, as a followee, a tagged user, the author of a referenced post or a mention, is indexed from their homeserver.
- Every user is read through one data model, the one their epoch names. A user on the 0.x model (`/pub/pubky.app/`) is read through the frozen 0.x reader of `pubky-social-specs`. Once the user's `social/v1` profile is written, the end of their migration, Nexus moves them to 1.x and reads only `social/v1` from then on.
- Data is validated through `pubky-social-specs`. A failure that can resolve itself, such as a missing dependency or a transient error, is retried. One that cannot, such as invalid data or an oversized resource, is not.

**Running it**

- One binary, `nexusd`. With no command, it runs the API, the watcher and the scheduled jobs, which is the Docker image's default. `nexusd api` and `nexusd watcher` run one each. `nexusd jobs` and `nexusd db` hold the on-demand jobs and the database commands.
- The configuration is `config.toml` in `~/.pubky-nexus/`, or the folder given with `--config-dir`. The watcher's homeserver fields are described in [docs/guides/homeservers.md](docs/guides/homeservers.md).
- It needs Neo4j 5.26 with the Graph Data Science plugin, Redis 8 with the search module, and ImageMagick for media. The Docker image includes ImageMagick. `docker/` holds a compose file for the databases.
- Telemetry is exported over OTLP when `[stack.otlp]` sets an `endpoint`.

## 0.4.1 and earlier

Versions 0.4.0 and 0.4.1 were published to crates.io without a tag in this repository. The tagged releases before them are on the [releases page](https://github.com/pubky/pubky-nexus/releases).
