# Referenced objects of other namespaces: design

Status: **design for review, not implemented.** Nothing in this document changes code, a wire
format, a graph label or an endpoint. "Decided" marks a direction this design takes; the
implementation of any of it is a separate change with its own review.

Code is cited as it stands on this branch. Where a route or field exists only on this branch
(it is added by the resource changes feed and external references changes beneath this
document, and is not on `main`), it is marked *branch*.

## 1. Rules

1. **Nexus does not watch other namespaces**, now or later. Seeing a homeserver event under
   `/pub/<namespace>/…` is not a reason to fetch, parse or index it.
2. An object of another namespace reaches Nexus only when social data references it: a tag in
   the social tag folder, or a social post's `parent`, `embed` or collection item.
3. Nexus never follows references found inside a referenced object.
4. Apps that need every object of their own namespace index them themselves; Nexus is not that
   indexer.
5. Social validation stays strict: social objects are read only with the social spec's closed
   kind sets, and nothing read from another namespace feeds social indexing.

## 2. Current behaviour

### 2.1 Routing

The watcher routes an event by its namespace segment (`nexus-watcher/src/events/route.rs`,
`route`) and decides once whether to fetch it (`nexus-watcher/src/events/classify.rs`,
`classify`): `pubky.app` (v0) and `social/v1` kinds Nexus models are indexed; tag files in
other apps' folders are skipped before any fetch; nothing else is an address Nexus indexes. The
pinned spec crate parses another namespace's path as `Resource::Foreign { namespace, version,
rest }`, "skipped by social readers".

### 2.2 References create Resources

A reference names a URI; Nexus normalises and hashes it (`universal_tag::normalize`) into one
`Resource` node `{id, uri, scheme, indexed_at}` and never fetches it.

| Reference (written by a social user) | Edge | Feed `kind` |
|---|---|---|
| tag at `pubky.app/tags/` or `social/v1/tags/` | `(:User)-[:TAGGED {label}]->(:Resource)` | `tag` |
| post `parent`, non-post URI (*branch*) | `(:Post)-[:REPLIED]->(:Resource)` | `reply` |
| post `embed`, non-post URI (*branch*) | `(:Post)-[:EMBEDS]->(:Resource)` | `embed` |
| collection item, non-post URI (*branch*) | `(:Post)-[:COLLECTED]->(:Resource)` | `collection` |

The feed's `kind` is the kind of **reference**. It is not, and must never be reused as, the
kind of the referenced object (section 4).

### 2.3 Social posts versus referenced targets

A social post is indexed from its author's own PUT; each edit (a new `social/v1` version) and
its DEL are events Nexus processes, so Nexus holds its current content, or a `deleted`
tombstone while relationships remain, or nothing after a hard delete.

A referenced target is never observed. Deleting a tag, reply or embed deletes that reference,
not the target, and a Resource disappears only when its last incoming reference goes. A
target's own edit or delete is an event under its namespace, which Nexus skips (2.1).

### 2.4 Target freshness (decided: consumers' scheduled refresh)

Consumers that show a target keep it current themselves. Pinakes re-reads each stored external
resource on a schedule, with a per-card TTL, backoff, a dead-letter state and an unreachable
state for `401`/`403` (not treated as deleted), and treats `404`/`410` as missing. This gives
**eventual** freshness while refreshes succeed. It is **not** an SLA: the TTL is a target, not a
bound; queueing, backoff, homeserver unavailability and dead-lettering delay an edit or delete
without limit until a refresh succeeds. No Nexus watcher and no known-URI event signal is added.

### 2.5 "Following" and reach (decided: keep current behaviour)

Everywhere Nexus filters by a social relation today, the relation applies to the **author**:

- `GET /v0/stream/posts?source=following|followers|friends&observer_id=` selects posts
  **authored** by those users (`queries/get.rs`, `post_stream`, the `Following`/`Followers`/
  `Friends` arms; the observer's own posts excluded).
- `GET /v0/search/posts/by_tag/{tag}?reach=&user_id=` selects posts tagged with the label
  whose **author** is in the reach (`models/post/search.rs`, `get_by_label_with_reach`).
- The resource streams have no reach parameter. Pinakes applies reach to **referrers** itself:
  it reads `GET /v0/user/{viewer}/following|friends` (limit 200) and filters its own references.

"Authored by someone I follow" and "tagged or embedded by someone I follow" are different
questions. This design keeps both behaviours exactly as they are and adds no following filter.
A referrer-reach filter in Nexus remains a possible later proposal, only if a consumer needs it.

## 3. Identity

### 3.1 Namespace + kind (decided)

A referenced object's type is the pair `(namespace, kind)`. `kind` is the **top-level string**
`kind` of the object's envelope; it is never a fixed `custom` kind with a `subkind` inside
`content`. The same kind name in two namespaces is two kinds: `review` in `alpha` and `review`
in `beta` never share a filter, a count or a validator.

### 3.2 What an address proves, and what it does not

- The key in `pubky://<key>/…` identifies the **account**, the public key whose signed
  discovery record points to a homeserver. It does not name the homeserver, and it is not
  evidence that the object exists.
- The namespace segment is a path. Any session the account authorised to write under `/pub/`
  can write any namespace there, so the namespace does not prove which app wrote the object or
  that its content follows that namespace's schema.
- Three different "authors" must stay apart: the **referrer** (the social user who tagged,
  replied or embedded, attested by their own signed write); the **address owner** (the key in
  the target's URI, derived from the address alone); and a **verified envelope author**, which
  exists only if the envelope was actually read from that account's homeserver and validated.

### 3.3 Spec compatibility

The base spec pin (social 1.0) has no envelope for other namespaces: such paths are
`Resource::Foreign` and nothing else. The social 1.1 draft adds sub-specs: an app's kinds use
the same `PostEnvelope<K>` as social, stored at `/{pub,priv}/<ns>/v1/posts/{id}/{editId}.json`
and referenced versionless as `pubky://{owner}/pub/<ns>/v1/posts/{id}`; `kind` is a top-level
string read through a catch-all (`Unknown(String)`) so a newer writer's kind survives a reader;
a namespace is 1–32 characters of `a-z0-9-` starting with a letter, and `social` is reserved
(`subspec::validate_namespace`). The 1.1 draft states that an indexer reads only the generic
envelope (namespace, kind, author, `parent`, `embed`) and nothing inside an app's `content`.

So the decided identity needs **no change** to the social envelope, the tag file or the
reference wire: it reads the sub-spec envelope the 1.1 draft already defines, once that draft is
released and pinned. Objects in a namespace that does not use the sub-spec layout have no
envelope kind; Nexus reports their kind as unknown and never guesses it from a path.

## 4. Graph model (decided: Resource, with referenced-only metadata)

Both models were compared on the same terms. Neither needs a watcher; both would read a target
only because it was referenced.

| | Resource + metadata (decided) | Post per referenced object |
|---|---|---|
| Identity | `id = hash(normalised URI)`, already shared by tags, replies, embeds | `(author, post_id)`; `post_id` is unique only per author **and namespace**, so every post key (graph match, `Post:Details:<author>:<id>` and every Redis index keyed by it) would need the namespace to avoid collisions with the author's social posts |
| New data | `namespace` (from the address), `kind`, `envelope_author`, `hydrated_at` on the node | the whole envelope: `content`, `kind`, `attachments`, `parent`, `embed`, versions, namespace |
| New edges | none | `AUTHORED` from the owner, thread edges for `parent`/`embed` |
| Content | none held | held, so served as if current |
| Lifecycle source | none needed: the node lives while referenced | edits and deletes would need events Nexus does not process (rule 1); without them Nexus serves stale content and never tombstones |
| Failure of a read | metadata stays unknown; the pointer still works | a post that cannot be read cannot be indexed, so a reference would point at nothing |
| API reuse | extends existing resource routes (views, streams, by-ids, counts, tags) | reuses post streams, but every social post route then needs a namespace filter by default so app objects do not enter social timelines |
| Cost | a few node properties and indexes | key migration across graph and Redis for every post |

**Decision: Resource.** It is the simplest model that gives a good API: one identity per URI
for everything that references it, no content Nexus would have to keep fresh, and no change to
social post keys or timelines. A Post representation is not ruled out by rule 1; it is ruled out
by its identity migration and by serving content whose edits and deletes Nexus cannot see.

### 4.1 Where the metadata comes from

- `namespace`: parsed from the URI at write time, no fetch. Only for `pubky://` URIs whose path
  passes the spec's namespace rule; `null` otherwise.
- `kind` and `envelope_author`: proposed from a **referenced-only hydration**: when a Resource is
  first created by a reference, and its URI is a versionless sub-spec post reference, read its
  newest version once and validate only the generic envelope. Not implemented and not part of any
  current change; it needs its own review because Nexus fetches no target today. Bounds: one read
  per new Resource; a byte cap; JSON depth and member caps; a time budget; no redirect off the
  account's homeserver; result stored as untrusted metadata with `hydrated_at`; failure leaves
  `kind` unknown with no retry storm (bounded backoff, then give up). No re-read on events (rule
  1); a re-read only on a later new reference after a minimum age. Nothing inside `content` is
  read or stored, and no URI found in the envelope is followed.
- Until hydration is approved, `kind` is always unknown.

### 4.2 Wire names

Two top-level strings on the Resource view, `namespace` and `kind`, both optional. `kind` never
appears without `namespace`. Bounds: `namespace` per the spec rule; `kind` up to 32 characters of
`a-z0-9_-`, otherwise reported as unknown and not filterable. The resource changes feed stays as
it is: its `kind` is the reference kind (2.2).

## 5. Validation

Content validation belongs to the app that owns the namespace. Nexus does not validate another
namespace's content and must never present it as validated. Nexus keeps its own safety envelope
on everything it stores or returns: URI length and scheme bounds, normalisation before hashing,
label bounds, the namespace and kind bounds in 4.2, cardinality caps so a writer cannot create
unbounded index entries, and the hydration bounds in 4.1.

## 6. Discovery and deletion limits

- Only references discover objects; an object nobody references is unknown to Nexus.
- Deleting a reference never means the target was deleted.
- Nexus has no "deleted" state for a Resource, because it never observes the target. A consumer
  that finds a target missing records that in its own state.
- A hydrated `kind` describes the object as read at `hydrated_at`, nothing later.

## 7. API

### 7.1 Current Resource routes

| Route | Where | Returns | Bounds |
|---|---|---|---|
| `GET /v0/resource/by-uri?uri` | main | one view | tag page bounds |
| `GET /v0/resource/{id}/tags`, `/tags/{label}/taggers` | main | tags, taggers | skip ≤ 10000, limit ≤ 100 |
| `GET /v0/stream/resources`, `/ids` | main | by latest tag time; `tags` (≤ 5, OR) | skip ≤ 10000, limit ≤ 100; `/ids` returns `last_score` |
| `GET /v0/resources/changes?cursor&limit` | *branch* | machine feed | limit 1–1000 |
| `POST /v0/stream/resources/by_ids` | *branch* | views for ids or URIs | 1–100, URI ≤ 2048 bytes |
| `GET /v0/resource/{id}/replies`, `/embeds` | *branch* | live referencing posts | skip ≤ 10000, limit 1–100 |

"main" means present on `main`; this document does not certify what any deployment runs.
`?app=` on the resource streams is rejected with 400 on this branch's base.

A view returns `details {id, uri, scheme, indexed_at}`, a tag page and `taggers_count`; on
*branch* also `replies_count`, `embeds_count`, `collections_count`. It has no namespace, kind,
author, title, existence or deletion field.

Consumers in use: Pinakes reads the feed (`limit=1000`, `(epoch, cursor)` committed with its
references), hydrates with `POST /v0/stream/resources/by_ids`, and reads follow lists for reach.
It also asks for `GET /v0/user/{viewer}/reach-set?depth=2`, which no Nexus route serves; it reads
the 404 as an empty set.

### 7.2 Returned fields, filter parameters, search routes

Three different things; a field in a view is not a filter, and a search route is not a stream
filter.

| Concept | Resource view field | Resource stream filter | Post equivalent today |
|---|---|---|---|
| Namespace | proposed | proposed `namespace=` | none (social only) |
| Kind | proposed (unknown until hydration) | proposed `kind=` with `namespace=` | stream filter `kind`, `exclude_kinds` (social kinds) |
| Address owner | proposed `address.owner` | proposed `owner=` | stream `source=author&author_id=` (verified author) |
| Verified envelope author | proposed, hydration only | not proposed | as above |
| Referrer (tagger) | `tags[].taggers` | not proposed (2.5) | none |
| Tags | `tags` | current `tags=` (≤ 5, OR) | stream `tags=` |
| Following / reach | — | not proposed (2.5) | stream `source=following…` (authored by); `search/posts/by_tag` `reach` (authored by) |
| Order | — | current latest tag time | stream `sorting=timeline|total_engagement` |
| Time bounds | `indexed_at` (creation) | current `start`/`end` on the score | stream `start`/`end` |
| Reference counts | *branch* counts | not proposed | counts in post views |
| Deletion state | none (6) | none | a `deleted` tombstone in a post view; **not** a filter |
| Content text | none | none | separate search route `GET /v0/search/posts/by_content` (social posts); not a stream filter |

### 7.3 Machine feed versus client queries

- **Machine feed** (*branch*): append-only, at-least-once, append-ordered, resumed by
  `(epoch, cursor)`; an epoch change means rebuild from zero. Never filtered: a filtered append
  log cannot be resumed without skipping lines. Bootstrap is reading from cursor 0 after a seed.
- **Client queries**: filtered and sorted, paged as in 7.4, with no completeness guarantee.

### 7.4 Proposed stable paging for filtered resource queries

Today's resource stream pages by `skip` or by an inclusive score bound with `skip=1`, which
skips or repeats on ties and is not bound to the filter. Proposed:

- Order by `(score DESC, id ASC)`; `id` is the Resource id, so ties are deterministic.
- `cursor` is opaque: the last `(score, id)` served plus a hash of the filter parameters. A
  request with a cursor returns rows strictly after it; a cursor presented with different
  filters is rejected with 400.
- Guarantees: no row is served twice **if its score does not change**, and ties never skip.
  Not guaranteed: a snapshot. A Resource re-scored by a new tag while a client pages may move
  ahead of the cursor (missed in this walk) or behind it (served again); a Resource whose last
  reference is deleted disappears. Clients that need completeness use the machine feed.
- Cost: today the score is `MAX(t.indexed_at)` aggregated over every `TAGGED` edge before
  ordering, under a 10-second budget. Keyset paging only pays off with a maintained
  `Resource.last_tagged_at` (set on tag put, recomputed on tag delete) and an index on it, plus
  an index on `Resource.namespace` so the namespace filter narrows first. Both need a
  backfill from existing edges.

### 7.5 Proposed requests and responses (examples)

```http
GET /v0/stream/resources/ids?namespace=alpha&kind=review&tags=coffee&limit=50
```

```json
{
  "resources": [{ "id": "19f876713c227a568d939946496a0d30", "score": 1792071211000 }],
  "cursor": "opaque"
}
```

```http
POST /v0/stream/resources/by_ids
{"uris": ["pubky://<key>/pub/alpha/v1/posts/0034A0X7NJ52C"]}
```

```json
[{
  "details": {
    "id": "19f876713c227a568d939946496a0d30",
    "uri": "pubky://<key>/pub/alpha/v1/posts/0034A0X7NJ52C",
    "scheme": "pubky",
    "indexed_at": 1792071211000,
    "address": { "owner": "<key>", "namespace": "alpha" },
    "kind": null,
    "envelope_author": null,
    "hydrated_at": null
  },
  "tags": [], "taggers_count": 0,
  "replies_count": 0, "embeds_count": 0, "collections_count": 0
}]
```

`address` is parsed from the URI and says nothing about existence, content or authorship.
`kind`, `envelope_author` and `hydrated_at` stay `null` until hydration (4.1) exists and has
read the envelope. There is no title, content or deletion field.

## 8. Decisions and remaining review items

Decided: no namespace watch (1); Resource graph model (4); top-level string kind scoped by
namespace (3.1); current author-based following kept, no new following filter (2.5); target
freshness by consumers' scheduled refresh, eventual and without an SLA (2.4).

For review with the implementation changes, not owner questions:

1. Approval of the one-read referenced-only hydration (4.1) and its bounds, since it is the
   first time Nexus would read a target.
2. Whether `namespace` and the keyset paging (7.4) land before hydration.
3. Whether Nexus should serve the reach-set route Pinakes asks for.
4. Releasing the social 1.1 spec, which the envelope read depends on (3.3).
