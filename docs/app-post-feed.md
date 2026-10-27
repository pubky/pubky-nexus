# Namespace post references

Any public `/pub/<ns>/v1/posts/{id}/{editId}[-slug].json` is read with the shared
`PostEnvelope<K>`. Namespace and kind tokens are generic; content and extensions
remain opaque. Social uses its own reader. Private, versionless and unsupported
paths are not stored events. Invalid envelopes produce no feed line.

Accepted PUT and DEL events append to `GET /v0/resources/changes` with `kind:"post"`.
`uri` is the versionless post URI, `resource_id` is its usual normalized Resource ID,
and `object_path` is the exact stored version file. A DEL removes that version's
reference, not every version of the logical post. A versionless directory DEL is
ignored. Retries can emit duplicate puts or deletes; use the feed's idempotent
reference key and apply lines in append order.

Nexus creates no app post node, edges or namespace stream. Social tags, replies,
embeds and collections can reference the same URI through the ordinary Resource
writers; their nodes survive an app version's deletion. The reference feed need
not have a corresponding Resource view until something references the URI.

The append and update of `Event:ResourceChanges:PostVersions` are one Redis
operation. The registry holds one line per accepted live version, allowing the
existing seed job to rebuild those lines without storing app content in Neo4j.
The existing cursor, epoch and concurrent-append rules apply. After total Redis
loss, graph seeding recovers social references; app version references require
replaying the homeserver objects, because they have no graph projection. Use
Redis persistence/backups to preserve both the feed and version registry.

No new endpoint or graph migration is required. Deploy the watcher and API together
so consumers can recognize the new enum value before accepting these events.
