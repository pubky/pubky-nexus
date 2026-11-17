# ADR 0001: Resource Reference Feed

## Status

Proposed — 2026-10-28

## Context

Consumers that build on Nexus's Resource references, such as Pinakes, need every put
and delete of a reference, in order, with a way to resume after a restart and to
rebuild after Redis is replaced. The resource streams cannot give that: they are
filtered, sorted and paged, with no replay guarantee, and they do not show deletes.
Hydrating the referenced Resources one request at a time is also too slow.

A tag reference is one graph edge per tagger, target and label, whichever file wrote
it: two spellings of one URL, or a tag's `pubky.app` file and its `social/v1` copy,
land on the same edge. When a user moves from `pubky.app` to `social/v1` (the flip),
Nexus rewrites the stored address of each tag edge they wrote, and its id where the
target's address changes too ([ADR 0003](0003-per-user-migration-to-social-v1.md)),
and nothing else about it.

## Decision

Nexus keeps an append-only feed of accepted references to generic Resources,
`GET /v1/resources/changes`, and a batch view to hydrate them,
`POST /v1/stream/resources/by_ids`. Nexus never fetches a referenced target. The
contract follows.

`GET /v1/resources/changes?cursor=0&limit=500` returns a JSON object with `lines` and
`next_cursor`. Each line describes an accepted reference to a generic Resource. Tags
and external post replies, embeds and collection items name Resources through this
feed. Native Post and User targets continue through their existing handlers. Tag
routing stays unchanged. The feed and the hydration route below are served under `/v1`
only: `/v0` is frozen and answers 404 for both.

```json
{"lines":[{"op":"put","resource_id":"19f876713c227a568d939946496a0d30","uri":"https://example.com/","kind":"tag","author":"<pk>","label":"example","object_path":"/pub/<folder>/tags/<id>","at":1792071211000}],"next_cursor":1}
```

`uri` is the normalized URI stored on the Resource and `resource_id` its id. For a tag,
`object_path` is the homeserver path of the tag file the line concerns: the file put,
or the file deleted or taken down. It is information only. Which tag folders reach
this feed is decided by the tag routing, so the example leaves the folder open.

A line's optional fields are absent when empty, and a consumer must ignore fields it
does not know: lines stay in the list across Nexus versions.

### Delivery contract

Lines are delivered **at least once, in append order**. A consumer keys a tag
reference by **(author, uri, label)**: it applies `put` as an upsert of that key and
`del` as a delete of it. Every file that lands on one edge publishes under the same
key, so equivalent files and the two folders' copies of a tag are one reference.
A post reference is keyed by **(author, object_path, kind, resource_id)** (see Post
references below). Duplicates are expected after retries and must be harmless: a
repeated put replaces the same reference, and deleting an absent key is a no-op. Do not
increment a counter for every line.

On a put the graph commit precedes the feed append. If the handler stops between
them, its retry finds the edge and appends the put. The put carries the edge's
indexed timestamp, which the graph write returns, so a retry, a replay or an
equivalent file appends the same `at`.

On a delete **the feed writes before the graph**. The handler reads the edge,
appends the del from it (the Resource's id and uri, the label), then deletes the
edge. If it stops between the two, the edge stays and its retry reads it again,
appends the del again and deletes it. Once the edge is gone the delete is a no-op. A
takedown goes through the same delete. A delete that never completes leaves a del
for a live edge until the seed job runs: the same failure mode as a put whose
retries run out between its graph write and its append, which leaves the graph
holding a change the feed lacks. The seed job (below) rebuilds the feed from the
graph in both cases. A seed that starts between a delete's append and its graph
delete reads the edge while the del is in the part of the list it replaces: the new
feed holds a put for an edge the delete then removes, until the next seed.

The watcher handles events concurrently from several tasks (the homeserver runners
and the retry processor). A resource tag's PUT, a tag DEL and a takedown hold one
lock per tagger across their graph write and their append, so the feed lists the
changes to one edge in the order the graph took them: every file that lands on the
edge is the same tagger's. The flip's tags step, which rewrites the tagger's edges,
holds the same lock for each batch from its read to its write, so a delete never
appends its del for an edge whose id the batch then changes. The lock is per watcher
process: this ordering holds for one watcher process per Neo4j and Redis, the
deployment the watcher's cursors already assume.

A post's references get the same ordering from one lock per post (author and post
id): a PUT of any of its version files and its DEL hold it from their first read
of the post's references to their last append. The post's references then match the
version handled last: no handler's references land between another one's post write
and its references, and a PUT cannot append an outbox after a concurrent DEL
published and retracted it. The handlers do not compare versions, so a PUT of an
older version handled after a newer one replaces both its content and references.

The flip appends nothing: it rewrites a tag edge's stored address, and its id where
the target's address changes, while the key, Resource and label stay the same. The
user's later `social/v1` delete retracts the same `(author, uri, label)` their
`pubky.app` put published.

`at` is the edge's indexed timestamp in Unix milliseconds for puts, and the
deletion's timestamp for dels. Apply lines in list order, not timestamp order.
`reply`, `embed` and `collection` identify references made by posts. Post replay may
refresh the reference timestamp; consumers still upsert the same key. The flip does not
move a post's references: each keeps the path of the post file that made it, and the
post's later edits and deletes retract that path.

The response carries `x-events-epoch` and `x-events-length`. This feed has its
own storage and epoch. Both headers are exposed through CORS. Epoch is opaque to
clients; length is the number of list entries. The page and headers come from one
atomic Redis read. Reads never create an epoch, append entries, query the graph, or
seed an empty list.

Persist the epoch and cursor together. On an epoch change, or when the **requested**
cursor exceeds the returned length, restart at zero and rebuild local reference
state. Do not compare a returned cursor with a length from another response.
Epoch is absent before the first append or seed; retain a previously saved epoch
when a header is absent. An epoch first appearing when none was saved is not itself
a reset signal. An empty/end page returns an empty array and the unchanged cursor.
Limits are 1–1000 (default 500); invalid values return 400.

A resource tag whose target, as written, is longer than the social/v1 spec's cap on a
reference URI (1024 code points) is rejected, whichever folder wrote it, which bounds
the size of its lines. The line carries the normalized URI, which percent-encoding can
make longer than the 2048 bytes the batch view accepts in `uris`: such a Resource is
hydrated by `resource_ids`.

### Deployment and rebuilding

Upgrade all watcher writers before seeding; older binaries do not append this feed.
Run the repository's migration command using the production database configuration:

```sh
nexusd --config-dir /path/to/config db migration run
```

`ResourceChangesSeed1792071211` is a single-stage migration, recorded as done after
a successful seed. As with other migrations, do not put a completed single-stage
migration in `backfill_ready`; the existing migration manager allows that setting
to explicitly force a backfill again. Other pending migrations run through the same
command and retain their own deployment requirements.

To intentionally rebuild later (for example after replacing/clearing Redis):

```sh
nexusd --config-dir /path/to/config jobs run resource-changes-seed
```

The job runs only on demand: every run resets every consumer, so a `cron` for it (or
for `resource-changes-seed-empty`) makes `nexusd run` and `nexusd jobs run` refuse
the config. It walks every User–TAGGED–Resource edge ordered
by `indexed_at`, with deterministic author/path/ID tie breakers, into a temporary
Redis list in batches of 1000, one put per edge under the edge's stored address. It
never renormalizes or changes IDs. An edge it cannot turn into a line (no stored
address, or one outside its author's folder) is logged and skipped, so one odd edge
cannot stop the seed or the migration that runs it. At publication, one Lua
operation copies the old list's live suffix after the snapshot, swaps the lists, and
increments the epoch counter. Thus watcher appends during the build survive. The
suffix can repeat snapshot references; the same consumer upsert rule handles this.
Readers see the old or the new generation. The final copy holds Redis for work
proportional to the live suffix, so prefer a quiet period for very large rebuilds.
The regular append path does not wait for the graph scan.

A snapshot with no lines does not replace a feed that held lines when the seed
started: an empty or wrong graph would otherwise reset every consumer. For a graph
whose references were really all deleted, `resource-changes-seed-empty` is the same
job allowed to publish the empty feed. The migration never does.

A shared seed lease prevents overlapping job/migration seeds; publication checks
ownership. The build has a one-hour deadline; temporary keys and the lease expire
if the process dies, leaving the old feed intact. A lease that cannot be released
after a publish is logged and expires by itself. The random initial numeric epoch
also distinguishes a newly initialized feed after Redis is cleared. After loss of
Redis state, run the seed job. The seed describes current graph references, not
historical deletions.

The list is `Event:ResourceChanges`, its epoch is `Event:ResourceChanges:Epoch`.
`Event:ResourceChanges:Seed:*` and `Event:ResourceChanges:SeedLock` are temporary.
This feed follows accepted tag events; it does not add hooks for unrelated graph
maintenance or profile-deletion cascades. Rebuilding reconciles current graph state.

### Hydrating references

`POST /v1/stream/resources/by_ids` accepts exactly one of `resource_ids` or `uris`
(1–100 entries) and optional `viewer_id`. URI entries are at most 2048 bytes and are
normalized and hashed as `GET /v1/resource/by-uri` does. Results are the usual
`/v1` resource views (each tag carries the viewer's `tag_uri` and no
`relationship`), in request order, retaining duplicates and skipping unknown
entries. Each entry includes `details.id`. Invalid inputs return 400 with the
reason; no matches returns 200 with an empty array. No remote fetch or resource
creation occurs. Use this with the feed to hydrate referenced IDs without a request
per resource.

### Post references

Social posts emit `reply`, `embed`, and `collection` for external targets. The target
URI uses the same normalization and ID as a tag. `object_path` is the actual stored
version file. One collection can emit multiple lines for that path.

Consumers of these kinds apply each line as an upsert or deletion keyed by
`(author, object_path, kind, resource_id)`. Lines remain at least once and in append
order; retries can duplicate a PUT. Keep distinct targets and kinds within one file.
An edit emits DEL for removed references or an older source-version key, then PUT
for each current reference. Deleting any social version follows the social-post
delete contract and retracts the indexed logical post's current references.

The edge update and a pending append batch commit in the same graph transaction.
Retries flush that batch before recording new changes. A delete follows the same rule as
a tag's: the feed writes before the graph. It publishes any pending batch, appends the
del of each of the post's current references, then deletes the post or makes it a
tombstone; a retry reads the references again and appends their dels again. A tombstone
keeps its Resource edges, which every Resource-side reader skips, and its later hard
delete appends nothing. Reseeding includes the Resource edges of live posts and retains
concurrent appends under the existing epoch/cursor rules. No external target is fetched.

## Consequences

### Positive ✅

- A consumer follows every reference change from a cursor, resumes after a restart and
  rebuilds from zero on an epoch change.
- Nexus never fetches a referenced target: the feed only reflects accepted writes.
- The key is the graph's identity of a tag, so the flip and equivalent files need no
  feed lines, records or sweeps of their own.
- A delete keeps no state outside the graph between its append and its graph delete.
- Many Resources are hydrated in one request.

### Negative ❌

- Delivery is at least once: consumers must upsert by key and accept duplicates.
- A handler whose retries run out between its graph write and its append leaves the
  feed and the graph apart until the seed job runs: a put missing from the feed, or a
  del for an edge still in the graph. So does a seed that starts between a delete's
  append and its graph delete or tombstone: a put for a reference already retracted.
- The per-tagger ordering holds for one watcher process per Neo4j and Redis.
- After loss of Redis state the feed must be reseeded, and a seed gives current
  references, not past deletions.

### Neutral ⚠️

- For a tag, `object_path` names the file a line concerns and is not part of its key:
  two lines of one tag reference can carry different paths.

## Alternatives Considered

### Polling the resource streams

They are filtered, sorted and paged, with no replay guarantee, and a consumer cannot
tell what was deleted.

### A filtered feed

A filtered view of an append log cannot be resumed without skipping lines. Filtered
client queries stay on the resource streams.

### Keying tags by file path

A consumer keyed by `(author, object_path)` loses track of a reference whenever the
graph rewrites the edge's address. The flip would need move lines (a put at the new
path and a del at the old one), markers on the edge until they are published, a
periodic sweep to retry them, and deletes aliased between the two addresses;
equivalent files would need an owner per edge. Keying by the graph's identity of a
tag needs none of that.

### A pending-delete journal

Recording a delete's line before the graph delete and publishing it after would keep
a del from ever reaching consumers while the edge is still in the graph. It needs a
Redis record per delete, a retry path that finds it after the edge is gone, and a
rule for a put that lands on the same key meanwhile. Writing before the graph has
one failure mode, a delete that never completes, which the seed job already repairs
for puts.
