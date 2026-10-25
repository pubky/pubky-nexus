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
URI, as written. Which tag folders reach this feed is the tag routing's to decide (another
app's folder today, `pubky.app/tags/` and `social/v1/tags/` after
[#1165](https://github.com/pubky/pubky-nexus/pull/1165)), so the example leaves the folder
open.

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

`at` is the edge's indexed timestamp in Unix milliseconds for puts (including
replays), and the deletion's indexed timestamp for dels. Apply lines in list order,
not timestamp order. `reply`, `embed` and `collection` are reserved kinds, never
emitted yet.

The response uses `x-events-epoch` and `x-events-length`, the header names proposed
for `/v0/events` in [PR #1125](https://github.com/pubky/pubky-nexus/pull/1125). This
feed has independent storage and does not require that PR. Both headers are exposed
through CORS. Epoch is opaque to clients; length is the number of list entries.
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
