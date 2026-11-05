# Resource reference feed

`GET /v0/resources/changes?cursor=0&limit=500` returns a JSON object with `lines` and
`next_cursor`. Each line describes an accepted reference to a generic Resource;
Nexus does not fetch the target. The first implementation emits only tags handled
by the universal-tag flow. Post and User targets continue through their existing
handlers and are excluded. This does not broaden which tag paths or targets Nexus
accepts, or add indexing of external post parents/embeds.

```json
{"lines":[{"op":"put","resource_id":"19f876713c227a568d939946496a0d30","uri":"https://example.com/","kind":"tag","author":"<pk>","label":"example","object_path":"/pub/<folder>/tags/<id>","at":1792071211000}],"next_cursor":1}
```

`object_path` is the homeserver path of the tag, reply or collection file that names the
URI, as written. Which tag folders reach this feed is decided by the tag routing, so the
example leaves the folder open.

## Delivery contract

Lines are delivered **at least once, in append order**. A consumer must apply `put`
as an upsert keyed by **(author, object_path)** and `del` as a delete by the same key.
Duplicates are expected after retries and must be harmless: a repeated put replaces
the same reference, and deleting an absent key is a no-op. Do not increment a
counter for every line.

The graph commit precedes the feed append. If the handler crashes between them,
a retry on the existing edge appends the missing put with its original indexed
timestamp. Retrying again after a successful append can emit an identical put;
the consumer's upsert leaves one reference. Pending deletes retain their metadata
until the graph deletion succeeds and publication is acknowledged, so a retry after
the edge is gone can still publish the del. Publication and pending record removal
are atomic. A later accepted put supersedes an unfinished delete for its path.

The watcher handles a tag file's events concurrently from several tasks (the
homeserver runners and the retry processor). A resource tag's PUT, its DEL and a
takedown of it take one lock per tag file (tagger and tag id) across their graph
write and their append, so the feed lists one file's changes in the order the
graph took them. The flip's tags step (below) takes the same locks for the edges
it moves, by each edge's tag id and the new id the flip gives it, from its read
of a batch to the batch's write, so no handler of those files reads an edge before
the move and appends after it. The publisher of moves (below) takes them too, for
each page of edges it publishes. A DEL or a takedown of an edge the flip gave a new
id holds the locks of both ids. The lock is per watcher process: this ordering
holds for one watcher process per Neo4j and Redis, the deployment the watcher's
cursors already assume. Equivalent tag files land on one edge and only its owner
publishes, so they need no shared lock.

What the feed cannot repair alone is a handler that stops for good between its
graph write and its append: a PUT or a DEL whose retries run out there. The graph
then holds a change the feed lacks. Running the seed job (below) rebuilds the feed from
the graph. One case around the flip (below) ends the same way: a `pubky.app` DEL that
deleted its edge but failed to publish, and whose user's flip completes before its
retry. The gate drops the retry once the user is at `social/v1`, so its del stays
unpublished until a delete of the tag's `social/v1` file. The seed job repairs it.

When a user moves from `pubky.app` to `social/v1` (the flip on their `social/v1`
profile), Nexus moves each resource tag edge they wrote to the address of its
`social/v1` tag file. Each move is published as a put at the new `object_path`
followed by a del at the old one, so a consumer ends with the reference under the
path the user's later `social/v1` delete retracts.

The move is recorded on the edge, in the same graph write that moves it: `v0_uri`
keeps the address the feed published the edge under, as stored, and `moved_at` is
set until the move is published. A rerun of that write over an edge already at its
new address keeps both. The moves are published outside the flip, which never fails
on the feed: once right after the flip's epoch write, and by a sweep the watcher
runs every minute over every edge whose move is still recorded, in pages of 100,
starting from the index on `TAGGED(moved_at)`, which holds only those edges. When
the first publish fails, a consumer sees the move up to one sweep later. Each move
is published under its tag file's lock, from the edge read again under it: the put
and the del are appended together, then `moved_at` is cleared if it is still the
move that was read, a page of 100 at a time under the page's locks. A crash between
the append and the clear makes the next sweep append the moves again, duplicates.

A tag deleted or taken down while its move is still recorded publishes a del of
`v0_uri` right after its own del: the consumer may still hold only that address.
That includes a tag on the user's own file, whose id the flip changes: the edge keeps
its `pubky.app` id beside the new one, and the delete of its `pubky.app` file finds it
by that id. Such a delete journals its del under the edge's address and names that
journal from the deleted file's address until the del is published, so a retry still
publishes it, whether it finds no edge or a tag made again at that file.

The del of `v0_uri` is pending with the delete's own, so a retry that no longer finds
the edge publishes both, and the move is never published afterwards, since its edge
is gone. A later put at the moved address, which supersedes the unfinished delete,
publishes the del of `v0_uri` pending with it, since the put does not bring that
address back. A retry that no longer finds the edge first checks, under the lock of
the file at `v0_uri` too, whether a tag is stored there again (made while the user
was still at `pubky.app`): then that del is dropped, since the address is live.

`at` is the edge's indexed timestamp in Unix milliseconds for puts (including
replays and moves), and the deletion's indexed timestamp for dels (the move's, for the
del a published move appends). Apply lines in list order,
not timestamp order. `reply`, `embed` and `collection` are reserved kinds, never
emitted yet.

The response carries `x-events-epoch` and `x-events-length`. This feed has its
own storage and epoch. Both headers are exposed through CORS. Epoch is opaque to clients; length is the number of list entries.
The page and headers come from one atomic Redis read. Reads never create an epoch,
append entries, query the graph, or seed an empty list.

Persist the epoch and cursor together. On an epoch change, or when the **requested**
cursor exceeds the returned length, restart at zero and rebuild local reference
state. Do not compare a returned cursor with a length from another response.
Epoch is absent before the first append or seed; retain a previously saved epoch
when a header is absent. An epoch first appearing when none was saved is not itself
a reset signal. An empty/end page returns an empty array and the unchanged cursor.
Limits are 1–1000 (default 500); invalid values return 400.

## Deployment and rebuilding

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

The job is unscheduled by default. It walks every User–TAGGED–Resource edge ordered
by `indexed_at`, with deterministic author/path/ID tie breakers, into a temporary
Redis list in batches of 1000. The stored tag URI supplies its path; legacy edges
without one use their stored app and tag ID. It never renormalizes or changes IDs.
At publication, one Lua operation copies the old list's live suffix after the
snapshot, swaps the lists, and increments the epoch counter. Thus watcher appends
during the build survive. The suffix can repeat snapshot references; the same
consumer upsert rule handles this. Readers see the old or the new generation.
The final copy holds Redis for work proportional to the live suffix, so prefer a
quiet period for very large rebuilds. The regular append path does not wait for the
graph scan.

A shared seed lease prevents overlapping job/migration seeds; publication checks
ownership. The build has a one-hour deadline; temporary keys and the lease expire
if the process dies, leaving the old feed intact. An empty graph still publishes
an empty feed with a new epoch. The random initial numeric epoch also distinguishes
a newly initialized feed after Redis is cleared. Redis persistence protects both
the list and pending delete records; after loss of Redis state, run the seed job.
The seed describes current graph references, not historical deletions.

The list is `Event:ResourceChanges`, its epoch is `Event:ResourceChanges:Epoch`.
`Event:ResourceChanges:PendingDelete:*` retains interrupted delete details;
`Event:ResourceChanges:PendingDeleteMoved:*` holds, beside the pending delete of a
tag whose move is not published, the del of the address it moved from;
`Event:ResourceChanges:PendingDeleteAlias:*` names, from a deleted tag file's address,
the pending delete of an edge stored under another address, and goes when that delete
is published. It can outlive that delete: when a later put lands at either address,
or the delete's retry is gated off. Resolving a leftover alias publishes only a pending
delete that still exists;
`Event:ResourceChanges:Seed:*` and `Event:ResourceChanges:SeedLock` are temporary.
This feed follows accepted tag events; it does not add hooks for unrelated graph
maintenance or profile-deletion cascades. Rebuilding reconciles current graph state.

## Hydrating references

`POST /v0/stream/resources/by_ids` accepts exactly one of `resource_ids` or `uris`
(1–100 entries) and optional `viewer_id`. URI entries are at most 2048 bytes and are
normalized and hashed as `GET /v0/resource/by-uri` does. Results are the usual
resource views, in request order, retaining duplicates and skipping unknown entries.
Each entry includes `details.id`. Invalid inputs return 400; no matches returns 200 with an
empty array. No remote fetch or resource creation occurs. Use this with the feed
to hydrate referenced IDs without a request per resource.
