# Versioning

Nexus 1.x serves `/v1` and reads the 1.x data model of [`pubky-social-specs`](https://github.com/pubky/pubky-social-specs). The two version numbers move independently.

A release is published as the Docker image `synonymsoft/pubky-nexus:vX.Y.Z`, with a GitHub release of the same name. What changed in each version is in its release notes, generated from the commits; how a release is cut is in [releasing.md](releasing.md).

## What a version number promises

From 1.0.0, the version follows [semver](https://semver.org) for the parts listed here, and only for those. A minor release adds, a patch release fixes, and a major release may remove or change.

| Area | Promise |
|---|---|
| `/v1` | Follows semver, as set out below. |
| `/v0` | Deprecated and frozen: fixes only, no new routes or fields. It can be removed in a 1.x minor release, announced in the README at least one release ahead. |
| Docker image | A `synonymsoft/pubky-nexus:vX.Y.Z` image is never pushed again once published. `latest` moves with each release. Each release is also tagged with its commit hash, which carries no promise beyond naming that build. |
| Everything else | Not covered: the config file, the `nexusd` commands, the Rust crates (not published) and the data Nexus stores in Neo4j and Redis. What an operator has to do for them still heads the release notes ([releasing.md](releasing.md)). |

## The `/v1` contract

A minor or patch release keeps, for every `/v1` route:

- the route and its method;
- its path and query parameters, the pagination parameters and their bounds;
- the response's field names and types;
- which fields can be missing and which are always there, and when a list comes back empty rather than as an error;
- the status codes.

Not part of the contract:

- the ranking and the content of the results, such as which posts a trust filter hides or the order of a ranked list;
- a default `limit`, within its bounds;
- the text of an error message.

A minor release may add routes, parameters and response fields, and new values to an enum in a response. A client ignores fields and enum values it does not know.
