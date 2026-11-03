# Indexing objects of other namespaces: design proposal

Status: **proposal for review**. Nothing here is implemented, and nothing here changes a wire
format, a graph label or an endpoint. Every "proposed" item below is a question for review,
not a decision. Sections marked *current* describe code as it stands.

Versioning on this carrier: `/v0` is frozen (fixes only). Existing `/v0` routes below
describe historical client behaviour; shared routes also have `/v1` mounts. The branch-only
feed, batch views and reference routes, their added view fields, and every proposed filter,
metadata field or paging extension in this document belong to `/v1` only. Consumers must
migrate these branch-only calls to `/v1`; this document does not claim a deployed client
has already migrated. No new behaviour is proposed for `/v0`.

## 1. The rule this proposal starts from

Nexus does not watch other namespaces, now or later. Seeing a homeserver event under
`/pub/<namespace>/…` is not a reason to fetch, parse or index it. An object of another
namespace reaches Nexus only when a social object references it: a tag in the social tag
folder, or a social post's reply, embed or collection item. Clients that need every object of
their own namespace index it themselves (or run an indexer for it); this proposal does not
settle who runs those indexers, and it does not turn Nexus into one.

Everything below works inside that rule. "Later namespace indexing" in this document means
richer handling of objects that social data already references, never discovery of objects
nobody referenced.

## 2. What exists today (current)

### 2.1 Event routing

The watcher routes an event by its namespace segment
(`nexus-watcher/src/events/route.rs`, `route`) and decides once whether to fetch it
(`nexus-watcher/src/events/classify.rs`, `classify`):

- `pubky.app/…` (v0) and `social/v1/…` kinds Nexus models are indexed.
- A tag file in another app's folder (`/pub/<app>/tags/{id}`) is skipped before any fetch,
  PUT and DEL alike.
- Any other path is not an address Nexus indexes. The pinned spec crate parses such a path as
  `Resource::Foreign { namespace, version, rest }`, "valid, classified, skipped by social
  readers".

### 2.2 How another namespace's object becomes a Resource

A reference names a URI; the URI is normalised and hashed (`universal_tag::normalize`) into a
`Resource` node `{id, uri, scheme, indexed_at}`. Nexus never fetches it. References are:

| Source (written by a social user) | Graph edge | Feed `kind` |
|---|---|---|
| tag at `pubky.app/tags/` or `social/v1/tags/` | `(:User)-[:TAGGED {label}]->(:Resource)` | `tag` |
| post `parent` naming a non-post URI | `(:Post)-[:REPLIED]->(:Resource)` | `reply` |
| post `embed` naming a non-post URI | `(:Post)-[:EMBEDS]->(:Resource)` | `embed` |
| collection item naming a non-post URI | `(:Post)-[:COLLECTED]->(:Resource)` | `collection` |

The resource changes feed (`GET /v1/resources/changes`) publishes each reference's put and
del, keyed by the referencing file (`author`, `object_path`, plus `kind` and `resource_id` for
post references).

### 2.3 Graph identity: Post versus Resource

| | `Post` | `Resource` |
|---|---|---|
| Identity | `(author, post_id)`; `uri` is its social address | `id = hash(normalised uri)` |
| Content | `content`, `kind`, `attachments`, `lock`, `deleted` | none: a pointer |
| Written by | the watcher, from the author's own social PUT | the first reference to its URI |
| Edited by | the author's later PUT (a `social/v1` version) | nobody: the URI is the identity |
| Deleted by | the author's DEL: hard delete, or a `deleted` tombstone while relationships remain | the last incoming reference going away |
| Kind | the social spec's post kind | none (`scheme` only) |

A `Resource` holds no envelope, kind, author or content. Nexus cannot tell from a Resource
whether its target exists, what it says, or which app wrote it.

### 2.4 Lifecycle: social posts versus referenced targets

- A social post is indexed from its author's PUT; its edits (new versions) and its DEL are
  events Nexus processes, so its view follows the author directly.
- A referenced target is not observed. Deleting a tag or a reply deletes that reference, never
  the target. A target's own edit or delete in its namespace reaches Nexus as an event Nexus
  skips (2.1).
- Consumers that show the target keep it current themselves. Pinakes, today, re-reads each
  stored external resource on a schedule (bounded TTL, backoff, dead-letter) and treats a
  `404`/`410` as missing and `401`/`403` as unreachable, not deleted. That gives **eventual**
  edit/delete propagation for referenced targets with no Nexus watcher, and no promptness
  guarantee.

## 3. Identity: namespace + kind (direction approved, representation open)

Identity of a typed object is the pair `(namespace, kind)`, never the kind alone: `reviews` in
`mapky` and `reviews` in `marketplace` are different kinds and must never share a filter,
counter or validator. Within that direction:

- **The namespace is a path segment, not a credential.** Anyone holding a write capability for
  a key's `/pub/` can write under any namespace there. A `pubky://<key>/pub/mapky/…` address
  proves only which key's homeserver holds it, if it exists. It does not prove that the mapky
  app wrote it, that its content follows mapky's schema, or that it exists at all.
- **A URI-derived owner is not an attested author.** The key in a `pubky://` URI names the
  homeserver; Nexus has not read the object. Any "owner" Nexus derives from a Resource URI must
  be labelled as derived from the address.
- **Kind is not derivable from a URI in general.** Some namespaces put a kind segment in the path
  (`/pub/<ns>/v1/<kinds>/<id>`); others do not. Nexus must not invent a kind from a path
  convention it was not told about.

## 4. Wire representation options (undecided)

These concern how a typed object of another namespace would be named on Nexus's wire (feed
lines, views, filters) if Nexus ever carries more than a pointer for it. None is chosen.

| Option | Shape | For | Against |
|---|---|---|---|
| A. `kind = resource` | One generic kind; the type lives only in the URI | No schema change; matches today's pointer model | No typed filtering; every client re-parses URIs |
| B. `kind = custom` + `subkind` | Fixed kind, free `subkind` string | Small enum stays closed | `subkind` without a namespace collides (`reviews`); two fields to keep consistent |
| C. namespace-scoped string kind | `{"namespace":"mapky","kind":"reviews"}` (two fields) or one string `mapky/reviews` | Matches the approved `(namespace, kind)` identity; collision-free by construction | Open set: Nexus needs bounds (length, charset, cardinality) and cannot validate meaning |
| D. promote to `Post` | Index the target as a `Post` with a foreign kind | Reuses post streams, counts, tags | Nexus would hold content it never fetched or cannot validate, and would need its lifecycle (section 6), which requires observing events this proposal rules out |

**Recommendation.** Keep the graph node a `Resource` (rule out D under the present rule), and if
typed filtering is wanted, use option C as two fields, `namespace` and `kind`, both optional and
both only ever derived from a source that states them. Until a source exists that states the
kind (section 6.3), only `namespace` (and the URI's key, labelled as address-derived) is
derivable, so the first step would be `namespace` alone.

## 5. Validation: whose job is what

Homeservers accept arbitrary writes, so content validation belongs to the app that owns the
namespace: it knows its schema and its rules. Nexus does not validate another namespace's
content and must not present it as validated.

Nexus remains responsible for its own safety envelope on anything it stores or returns:

- URI length and scheme bounds (today: 2048 bytes and a valid scheme on the batch route),
  normalisation and hashing before storage.
- Label and identifier bounds on tags (unchanged).
- If a snapshot is ever fetched (6.3): a byte cap, a JSON depth/field cap, a time budget, no
  redirects off the target host, no active content, and the snapshot stored as untrusted data.
- Cardinality caps on any open string field (namespace/kind) so a writer cannot create
  unbounded index entries.

## 6. Lifecycle and discovery (limits and alternatives)

### 6.1 Discovery

Only references discover objects (2.2). A namespace object nobody references is unknown to
Nexus, by design. Nexus does not follow references found inside a referenced object either: no
automatic follow-on watching.

### 6.2 Edits and deletes of a referenced target

Not observed (2.4). The current consumers' scheduled exact-target refresh provides eventual
updates. Alternatives, none authorised:

| Alternative | What it adds | Cost / risk |
|---|---|---|
| Nothing (today) | — | Staleness bounded by each consumer's refresh |
| Exact-known-URI event filter | On an event whose URI exactly equals a stored Resource URI, publish a "target changed" signal (no fetch) | One lookup per skipped event; still no content; must never widen to prefix or namespace matches |
| Snapshot on reference (6.3) | Content at reference time | Fetch budget, safety, stale snapshot semantics |

### 6.3 Optional referenced-target snapshot (later, not proposed for now)

A later stage could fetch a referenced target once, when its first reference arrives, to
derive `kind` or a title. If ever approved it must be: optional per deployment, behind an
allowlist of namespaces, budgeted (rate, bytes, time), stored as untrusted, never re-fetched by
event (only by a bounded schedule), and never a source of further discovery.

### 6.4 Deletion semantics to keep

- Deleting a reference never means the target was deleted.
- A Resource disappears when its last incoming reference goes, not when its target goes.
- A target the consumer finds missing is the consumer's state (`missing`/`unreachable`), not
  Nexus's; Nexus has no "deleted" state for a Resource because it never observes the target.

## 7. API surface

### 7.1 Current endpoints that serve Resources (current)

| Endpoint | What it returns | Bounds |
|---|---|---|
| `GET /v1/resources/changes?cursor&limit` | Machine feed of reference puts/dels; headers `x-events-epoch`, `x-events-length` | limit 1–1000 (default 500) |
| `POST /v1/stream/resources/by_ids` | Resource views for `resource_ids` or `uris`, request order, unknown skipped | 1–100 entries, URI ≤ 2048 bytes |
| `GET /v0/resource/by-uri?uri` | One Resource view | tag page bounds |
| `GET /v0/resource/{id}/tags` | Tags on a Resource | `skip_tags` ≤ 10000, `limit_tags` ≤ 100, `limit_taggers` ≤ 100 |
| `GET /v0/resource/{id}/tags/{label}/taggers` | Taggers of one label | skip ≤ 10000, limit ≤ 100 |
| `GET /v1/resource/{id}/replies`, `/embeds` | Live posts referencing the Resource, newest first | skip ≤ 10000, limit 1–100 (default 20) |
| `GET /v0/stream/resources`, `/ids` | Resources by latest tag time, optional `tags` (≤ 5, OR) | skip ≤ 10000, limit ≤ 100; `?app=` is rejected with 400 |

A Resource view carries `details {id, uri, scheme, indexed_at}`, a tag page, `taggers_count`,
`replies_count`, `embeds_count` and `collections_count`. There is no namespace, kind, owner,
title, existence or deletion field, because Nexus has no source for them.

On this branch the machine consumer (Pinakes) must read `GET /v1/resources/changes` with `limit=1000`, persists
`(epoch, cursor)` with the references in one transaction, and hydrates with
`POST /v1/stream/resources/by_ids` (`uris`, optional `viewer_id`). For reach it reads
`GET /v0/user/{viewer}/following|friends` (limit 200) and asks
`GET /v0/user/{viewer}/reach-set?depth=2`, which no Nexus build in this series serves; that
client treats the 404 as an empty set.

### 7.2 Machine feed versus client queries

These are different contracts and must stay separate:

- **Machine feed** (`/v1/resources/changes`): append-only, at-least-once, ordered by append,
  resumable by `(epoch, cursor)`; an epoch change means rebuild from zero. It is not filtered:
  a filtered view of it could not be resumed without skipping unseen lines. Bootstrap is reading
  it from cursor 0 after a seed.
- **Client queries** (streams below): filtered, sorted, paged by `skip`/`limit` or a score
  bound, with no completeness or replay guarantee.

### 7.3 Filter matrix

S = supported today, P = proposed here (not implemented), N/A = not applicable or ruled out.

| Filter | Resource stream | Resource replies/embeds | Post stream (for comparison) |
|---|---|---|---|
| Namespace (from URI) | P | P (on the Resource) | N/A (social posts only) |
| Kind within namespace | P, only once a source states it (4, 6.3) | P, same | S (social kinds only) |
| Target owner (URI key, address-derived) | P, labelled as derived | P | S (`author_id`, attested by the author's own write) |
| Referrer / tagger | P (`tagger_id`) | S implicitly (the posts are the referrers); P `author_id` filter | S (`source=author`) |
| Tags (label) | S (`tags`, ≤ 5, OR) | N/A | S |
| Reach: following / friends of a viewer (applies to referrers) | P | P | S (`source=following|friends`, `observer_id`) |
| Web-of-trust reach | P, later | P, later | S (`source=wot`, `wot_domain`) |
| Order: latest reference time | S (`timeline`) | S (newest first) | S |
| Order: reference count | P | N/A | S (`total_engagement`) |
| Time bounds | S (`start`/`end` on score) | P | S |
| Pagination | S (skip ≤ 10000, limit ≤ 100, `last_score` on `/ids`) | S (skip ≤ 10000, limit ≤ 100) | S |
| Content search | N/A (no content) | N/A | S (social posts only) |
| Deletion state of target | N/A (not observed) | N/A | S (`deleted` tombstone) |

"Author" always means two different things and the API must name them apart: the **referrer**
(the social user who tagged, replied or embedded, attested by their own write) and the
**target owner** (the key in the target's address, not attested).

### 7.4 Proposed requests and responses (examples only)

Generic extensions of the existing resource stream, not app-specific endpoints:

```http
GET /v1/stream/resources/ids?namespace=mapky&tags=coffee&reach=following&observer_id=<pk>&limit=50
```

```json
{
  "resource_ids": ["19f876713c227a568d939946496a0d30"],
  "last_score": 1792071211000
}
```

`namespace` matches the Resource's URI namespace segment exactly (bounded charset/length; 400
otherwise). `reach` restricts the taggers counted to the observer's following or friends; the
score becomes the latest such tag. `kind` would be accepted only once a stated-kind source
exists, and `namespace` would be required with it.

Hydration stays `POST /v1/stream/resources/by_ids`. A view could gain address-derived fields,
clearly named:

```json
{
  "details": {
    "id": "19f876713c227a568d939946496a0d30",
    "uri": "pubky://<key>/pub/mapky/v1/reviews/0034A0X7NJ52C",
    "scheme": "pubky",
    "indexed_at": 1792071211000,
    "address": { "owner": "<key>", "namespace": "mapky", "path": "v1/reviews/0034A0X7NJ52C" }
  },
  "tags": [],
  "taggers_count": 0,
  "replies_count": 0,
  "embeds_count": 0,
  "collections_count": 0
}
```

`address` is parsed from `uri` and says nothing about existence, content or authorship. There
is deliberately no `kind`, `title` or `deleted` field: no source states them.

Per-referrer filtering on the existing reference routes:

```http
GET /v1/resource/{id}/replies?author_id=<pk>&limit=20
```

### 7.5 Index and cost bounds

- Today's resource stream aggregates `MAX(t.indexed_at)` over every `TAGGED` edge to a
  Resource before ordering, under a 10-second query budget; its cost grows with all resource
  tags. A namespace filter on it would need a `Resource.namespace` property (derived at write
  from the URI, backfilled from `r.uri` by a migration) and an index on it, so the filter
  narrows before the aggregate.
- Reach filters bind the observer's follow set first (bounded like the existing follow routes)
  and then expand their tags; they need no new index beyond `taggedTimestampIndex`.
- Every new open string (namespace, later kind) needs a length and charset bound, and a cap on
  distinct values per deployment, to keep index cardinality bounded.
- None of this changes the machine feed, which stays unfiltered.

## 8. Questions for the owner

1. Is scheduled exact-target refresh in consumers enough for target edits/deletes, or should
   the exact-known-URI signal (6.2) be designed? Either way, no namespace watch.
2. Is `namespace` alone (address-derived) useful enough to add before any kind source exists?
3. If a kind is ever needed: from which source — a snapshot (6.3), the referencing social
   object stating it, or never?
4. Who runs full indexes for clients that need every object of their namespace?
5. Which filters in 7.3 does Pinakes need first, and in which order?
6. Should the reach-set route Pinakes asks for exist in Nexus, or should Pinakes rely on the
   follow lists alone?

## 9. Recommendation

Keep Resources as pointers and keep the no-watch rule absolute. Approve, if anything, only the
address-derived `namespace` filter and the referrer reach filter on the existing resource
stream, with the bounds in 7.5, as a separate reviewed change. Defer kinds until a stated-kind
source is agreed, and leave target lifecycle to consumers' refresh unless the exact-known-URI
signal is approved.
