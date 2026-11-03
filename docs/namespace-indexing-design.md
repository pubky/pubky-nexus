# Referenced objects of other namespaces: design

Status: **design for review, not implemented.** Nothing in this document changes code, a wire
format, a graph label or an endpoint. "Decided" marks a direction this design takes; the
implementation of any of it is a separate change with its own review.

Code is cited as it stands on this branch. Where a route or field exists only on this branch
(it is added by the resource changes feed and external references changes beneath this
document, and is not on `main`), it is marked *branch*.

Versioning on this carrier: `/v0` is frozen (fixes only). Existing `/v0` routes below
describe historical client behaviour; shared routes also have `/v1` mounts. The branch-only
feed, batch views and reference routes, their added view fields, and every proposed filter,
metadata field or paging extension in this document belong to `/v1` only. Consumers must
migrate these branch-only calls to `/v1`; this document does not claim a deployed client
has already migrated. No new behaviour is proposed for `/v0`.

## 1. Rules

1. **Nexus does not watch other namespaces**, now or later. Seeing a homeserver event under
   `/pub/<namespace>/…` is not a reason to fetch, parse or index it.
2. An object of another namespace reaches Nexus only when social data references it: a tag in
   the social tag folder, or a social post's `parent`, `embed` or collection item.
3. Nexus never follows references found inside a referenced object.
4. Who runs a full index for clients that need every object of a namespace is outside this
   design. The only fixed point is rule 1: it is not Nexus discovering namespaces on its own.
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

### 2.5 Following, reach and web of trust (decided: keep current behaviour exactly)

Routes that take a social relation apply it to different subjects. Each row is current
behaviour on this branch's base, and every one is kept as it is. `reach` is `followers |
following | friends | wot | wot_1..wot_3` (bare `wot` is depth 2) anchored on the named user.

| Route and parameters | Who the relation selects | Source |
|---|---|---|
| `GET /v0/stream/posts?source=following\|followers\|friends&observer_id=` | post **authors** in that relation to the observer (observer excluded) | `queries/get.rs` `post_stream`, `Following`/`Followers`/`Friends` arms |
| `GET /v0/stream/posts?source=wot&observer_id=&depth=1..3` | post **authors** within `depth` follow steps (observer excluded) | `post_stream`, `Wot` arm |
| `GET /v0/stream/posts?source=wot_domain&observer_id=&domain_tags=&depth=0..3` | post **authors** endorsed with a `domain_tags` label by **taggers** in the observer's follow reach (`depth=0`: by the observer alone) | `post_stream`, `WotDomain` arms |
| `GET /v0/search/posts/by_tag/{tag}?user_id=&reach=` | tagged posts whose **author** is in reach (never `user_id`) | `models/post/search.rs` `get_by_label_with_reach` |
| `GET /v0/search/posts/by_content?user_id=&reach=` | matching posts whose **author** is in reach (never `user_id`) | `routes/v0/search/posts.rs` |
| `GET /v0/tags/hot?user_id=&reach=` | tags whose **tagger** is in reach | `queries/get.rs` `get_hot_tags_by_reach` |
| `GET /v0/tags/taggers/{label}?user_id=&reach=` | **taggers** in reach who used the label on posts | `queries/get.rs` `get_tag_taggers_by_reach` |
| `GET /v0/search/users/by_tags?user_id=&reach=` | **tagged users** in reach (never `user_id`), scores unchanged | `queries/get.rs` `search_users_by_tags_with_reach` |
| `GET /v0/stream/users?source=influencers&user_id=&reach=` | **users** in reach, ranked as influencers | `models/user/stream.rs` |
| `GET /v0/post/{a}/{id}/tags`, `GET /v0/user/{id}/tags`, `/taggers/{label}`, `GET /v0/user/{id}` with `viewer_id=&depth=1..3` | **taggers** of those tags within `depth` follow steps of the viewer (the post taggers route ignores `depth`) | `queries/get.rs` `get_viewer_trusted_network_tags`, `get_viewer_trusted_network_post_tags` |
| `GET /v0/stream/resources`, `/ids`, Resource routes | no relation parameter | `routes/v0/stream/resources.rs` |

Pinakes applies reach to **referrers** of Resources itself: it reads
`GET /v0/user/{viewer}/following|friends` (limit 200) and filters its own references.

"Authored by someone I follow" and "tagged or embedded by someone I follow" are different
questions, and both already exist on different routes. This design keeps every row above as it is
and adds no relation filter and no new meaning for an existing one. A relation filter on the
resource streams would be a separate future proposal.

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

Two spec pins matter: this branch's base pins `pubky-social-specs` at `acde362` (crate version
`1.0.0-alpha.5`); the separate social 1.1 work pins the draft `b2916840` (`1.1.0-alpha.1`).
Neither is a release.

**Already in the base pin (`acde362`).**

- The generic envelope `PostEnvelope<K: PostKind>`, the object "every specialization shares",
  stored at `{root}/{namespace}/v1/posts/{id}/{editId}.json`; its documentation says a namespace
  can ship its own kind set on the same envelope and that social readers classify those paths as
  foreign (`src/models/post/mod.rs`, `PostKind` and `PostEnvelope`).
- The path builder `namespace_path(root, namespace, leaf)` (`src/constants.rs`) and
  `PostEnvelope::version_path(namespace, …)`, `create_version_under`, `edit_version_under`, which
  refuse the reserved `social` namespace (`reject_social_namespace`).
- `kind` as the envelope's top-level string, read into a `PostKind` whose catch-all
  (`is_known() == false`) "reads, and fails validation".
- A generic `Validatable` for `PostEnvelope<K>` and `check_references(ctx, owner)`, the reference
  gate with the author in scope.
- `ParsedUri` classifying another namespace's path as `Resource::Foreign { namespace, version,
  rest }` (`src/uri/parsed.rs`).

**Added by the 1.1 draft (`b2916840`), not in the base pin.**

- A `subspec` module (`src/subspec.rs`): `validate_namespace` (1–32 characters of `a-z0-9-`,
  starting with a letter, `social` reserved), `version_path`, the versionless reference
  `post_uri` (`pubky://{owner}/{root}/{ns}/v1/posts/{id}`), `parse_post_uri`, `read_version`
  (validate a stored version with the ownership rule), the `subspec_names!` macro and vector
  files.
- A `check_owner` hook on `PostKind`.
- `docs/SUBSPECS.md`, which states that an indexer reads only the generic envelope (namespace,
  kind, author, `parent`, `embed`) and nothing inside an app's `content`.

**In neither pin.** A way for an indexer to read the kind of **any** namespace. Every `PostKind`
is one namespace's closed set: an unknown name fails validation, and `read_version::<K>` needs a
`K`. Reading arbitrary namespaces needs:

- a namespace-agnostic indexer kind type that accepts any well-formed kind name and has no content
  rules, so the envelope's own rules (id, `extra`, list caps, the reference gate, the ownership
  rule) still run;
- a kind-name syntax, which neither pin defines for other namespaces.

Proposed: both go into the spec crate, so every indexer agrees on a well-formed kind. A
Nexus-local implementation of the public `PostKind` trait would work against either pin, but
would make Nexus the owner of a naming rule the spec should own. Neither changes the envelope
wire, the social envelope, tag files or reference positions.

**Release dependencies.** Reading the envelope needs nothing beyond the base pin's types; the
namespace syntax rule and the reference parser need the 1.1 draft's `subspec` module in a
released spec; the indexer kind type and kind-name syntax need a spec change not yet written.

Social validation stays strict: social posts keep the closed `PubkySocialPostKind`, the generic
type is never used for `social` (both pins reserve it), and nothing it reads enters social
indexing. Objects in a namespace that does not use this layout have no envelope kind; Nexus
reports their kind as unknown and never guesses it from a path.

## 4. Graph model (decided: Resource, with metadata from the address and, only if approved, from one read)

Both models were compared on the same terms. Neither needs a watcher: either would get a target's
metadata or content only from a read made because it was referenced (4.1, unapproved), and either
would see a target's later edits and deletes only through such a read.

| | Resource + metadata (decided) | Post per referenced object |
|---|---|---|
| Identity | `id = hash(normalised URI)`, already shared by tags, replies, embeds | `(author, post_id)`; `post_id` is unique only per author **and namespace**, so every post key (graph match, `Post:Details:<author>:<id>` and every Redis index keyed by it) would need the namespace to avoid collisions with the author's social posts |
| New data | `namespace` (from the address), `kind`, `envelope_author`, `hydrated_at` on the node | the whole envelope: `content`, `kind`, `attachments`, `parent`, `embed`, versions, namespace |
| New edges | none | `AUTHORED` from the owner, thread edges for `parent`/`embed` |
| Content | none held | held, and served by post routes as a post's current content |
| Lifecycle | the node lives while referenced; its metadata is as old as the last read | the post's content, attachments and thread edges are as old as the last read; a deleted target stays a live post until a read finds it gone, which post routes cannot show |
| Failure of a read | metadata stays unknown; the pointer still works | either no node, so the reference points at nothing, or a placeholder post with no content, a new state every post route must handle |
| API reuse | needs new `namespace`, `kind`, `owner` filters on the resource streams (7) | post filters (`kind`, `source=author`, `tags`, `following`) apply at once, but every social route then needs a default namespace filter so app objects stay out of social timelines, counts and notifications |
| Cost | a few node properties, two indexes, a backfill from `uri` | namespace in every post key across graph and Redis, and the default filter in every post query |

**Decision: Resource.** It is the simplest model that gives a good API: one identity per URI for
everything that references it, metadata whose age is explicit, and no change to social post keys,
timelines or counts. The Post model is not rejected for needing a watcher; it needs none. It is
rejected for the post-key migration and for serving referenced content through routes that
present it as current.

### 4.1 Where the metadata comes from

- `namespace`: parsed from the URI at write time, no fetch. Only for `pubky://` URIs whose
  namespace segment passes the namespace syntax rule (the 1.1 draft's `validate_namespace`, 3.3);
  `null` otherwise.
- `kind` and `envelope_author`: only from a **referenced-target hydration**, which is an
  **unapproved, future, owner-scoped implementation**. This design describes its shape; neither
  this design nor its review authorises it, because Nexus reads no target today. If the owner
  ever approves it: when a Resource is first created by a reference and its URI is a versionless
  post reference under the shared layout, list that post's own folder newest first with a limit
  of one, read that version once and validate only the generic envelope with the indexer kind
  type (3.3). Bounds: one read per new Resource; a byte cap; JSON depth and member caps; a time
  budget; no redirect off the account's homeserver; stored as untrusted metadata with
  `hydrated_at`; failure leaves `kind` unknown, with bounded backoff and then give up. No re-read
  on events (rule 1); at most a re-read on a later new reference after a minimum age. Nothing
  inside `content` is read or stored, and no URI found in the envelope is followed.
- Without that approval, `kind` and `envelope_author` are never set.

### 4.2 Wire names

All proposed metadata sits in the Resource view's `details`, beside today's `id`, `uri`,
`scheme` and `indexed_at`: `namespace` and `address_owner` (from the URI), and `kind`,
`envelope_author` and `hydrated_at` (only from an approved hydration). All are optional; `kind`
never appears without `namespace`. Bounds: `namespace` per the namespace syntax rule; `kind` per
the kind-name syntax the spec is asked to define (3.3), and until then no `kind` is stored at
all. A value that fails its rule is reported as unknown and is not filterable. The resource
changes feed stays as it is: its `kind` is the reference kind (2.2).

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
| `GET /v1/resources/changes?cursor&limit` | *branch* | machine feed | limit 1–1000 |
| `POST /v1/stream/resources/by_ids` | *branch* | views for ids or URIs | 1–100, URI ≤ 2048 bytes |
| `GET /v1/resource/{id}/replies`, `/embeds` | *branch* | live referencing posts | skip ≤ 10000, limit 1–100 |

"main" means present on `main`; this document does not certify what any deployment runs.
The resource streams' `?app=` differs: on `main` it filters by the app folder the **tag file**
was written in (the referrer's side); on this branch's base it is rejected with 400, since tags
in other apps' folders are no longer indexed. Neither is a filter on the target's namespace.

A view returns `details {id, uri, scheme, indexed_at}`, a tag page and `taggers_count`; on
*branch* `/v1` views also `replies_count`, `embeds_count`, `collections_count`. It has no
namespace, kind, author, title, existence or deletion field.

The separate, dependent social 1.1 change adds `sorting=rating` to the `/v1` resource streams
and `rating_avg`/`rating_count` to `/v1` Resource views. That is its own scope, not the baseline
of this design, and nothing here depends on it.

Consumer contract after migration to this branch: Pinakes reads the feed (`limit=1000`,
`(epoch, cursor)` committed with its references), hydrates with
`POST /v1/stream/resources/by_ids`, and reads follow lists for reach.
Separately, the current Pinakes client also asks for `GET /v0/user/{viewer}/reach-set?depth=2`,
which no Nexus route serves; it reads the 404 as an empty set.

### 7.2 Returned fields, filter parameters, search routes

Three different things; a field in a view is not a filter, and a search route is not a stream
filter. The post column lists parameters of `GET /v0/stream/posts` and the separate search
routes as they exist on `main` and this branch's base.

| Concept | Resource view field | Resource stream filter | Post equivalent today |
|---|---|---|---|
| Namespace | proposed | proposed `namespace=` | none (social only) |
| Kind | proposed (only after an approved hydration) | proposed `kind=` with `namespace=` | stream filter `kind`, `exclude_kinds` (social kinds) |
| Address owner | proposed `address_owner` | proposed `owner=` | stream `source=author&author_id=` (the post's own author) |
| Verified envelope author | proposed, only after an approved hydration | not proposed | as above |
| Referrer (tagger) | `tags[].taggers` | not proposed (2.5) | tagger reach exists on tag routes, not on posts (2.5) |
| Tag file's app folder | — | `app=` on `main` only; rejected on this branch's base (7.1) | none |
| Tags | `tags` | current `tags=` (≤ 5, OR) | stream `tags=` |
| Following / reach / web of trust | — | not proposed (2.5) | stream `source=following\|followers\|friends\|wot` (authors), `wot_domain` (authors endorsed by trusted taggers), search `reach` (authors); full table in 2.5 |
| Order | — | current latest tag time | stream `sorting=timeline|total_engagement` |
| Time bounds | `indexed_at` (creation) | current `start`/`end` on the score | stream `start`/`end` |
| Reference counts | *branch* `/v1` counts | not proposed | counts in post views |
| Deletion state | none (6) | none | a `deleted` tombstone in a post view; **not** a filter |
| Content text | none | none | separate search route `GET /v0/search/posts/by_content` (social posts); not a stream filter |

### 7.3 Machine feed versus client queries

- **Machine feed** (*branch*): append-only, at-least-once, append-ordered, resumed by
  `(epoch, cursor)`; an epoch change means rebuild from zero. Never filtered: a filtered append
  log cannot be resumed without skipping lines. Bootstrap is reading from cursor 0 after a seed.
- **Client queries**: filtered and sorted, paged as in 7.4, with no completeness guarantee.

### 7.4 Proposed stable paging for filtered resource queries

Today the resource stream orders by `score <order>, r.id <order>` (descending by default, on
`main` and this branch's base). Two ways of paging it exist, with different properties:

- `skip`/`limit` over an unchanged result set is already deterministic on ties, because the
  order includes `r.id`; it is capped at `skip ≤ 10000` and shifts when rows are added or
  removed.
- Resuming from `/ids`'s `last_score` with an inclusive `start` cannot tell tied rows apart:
  `last_score` carries no id, so the client must also `skip` every row already served at that
  score (`skip=1` assumes no ties). It is not bound to the filters either.

Proposed:

- Keep the current order, `(score <order>, id <order>)`, so existing clients see tied rows in the
  same order.
- `cursor` is opaque: the last `(score, id)` served, the order, and a hash of the filter
  parameters. A request with a cursor returns rows strictly after it in that order; a cursor presented with different
  filters, or together with `skip`, is rejected with 400. `limit` keeps its current bound
  (1–100); a cursor walk has no depth limit because each page is one index seek, unlike `skip`,
  which stays capped at 10000.
- Compatibility: additive. `skip`, `start`, `end` and the current `/ids` fields
  (`resource_ids`, `last_score`) stay; `/ids` gains `cursor`, and both routes accept it.
- Guarantees: a row whose score does not change while the client pages is served exactly once,
  and ties never skip or repeat. Not guaranteed: a snapshot or completeness. A Resource
  re-scored by a new tag may move ahead of the cursor (missed in this walk) or behind it (served
  again); a Resource first referenced after the walk began scores ahead of it and is not served;
  a Resource whose last reference is deleted disappears. Clients that need every change use the
  machine feed.
- Cost: today the score is `MAX(t.indexed_at)` aggregated over every `TAGGED` edge before
  ordering, under a 10-second budget. Keyset paging only pays off with a maintained
  `Resource.last_tagged_at` (set on tag put, recomputed on tag delete) and an index on it, plus
  an index on `Resource.namespace` so the namespace filter narrows first. Both need a
  backfill from existing edges.

### 7.5 Proposed requests and responses (examples)

```http
GET /v1/stream/resources/ids?namespace=alpha&kind=review&tags=coffee&limit=50
```

```json
{
  "resource_ids": ["19f876713c227a568d939946496a0d30"],
  "last_score": 1792071211000,
  "cursor": "opaque"
}
```

The next page repeats the same filters with `&cursor=<cursor>`.

```http
POST /v1/stream/resources/by_ids
{"uris": ["pubky://<key>/pub/alpha/v1/posts/0034A0X7NJ52C"]}
```

```json
[{
  "details": {
    "id": "19f876713c227a568d939946496a0d30",
    "uri": "pubky://<key>/pub/alpha/v1/posts/0034A0X7NJ52C",
    "scheme": "pubky",
    "indexed_at": 1792071211000,
    "namespace": "alpha",
    "kind": null,
    "address_owner": "<key>",
    "envelope_author": null,
    "hydrated_at": null
  },
  "tags": [], "taggers_count": 0,
  "replies_count": 0, "embeds_count": 0, "collections_count": 0
}]
```

`namespace` and `address_owner` are parsed from the URI and say nothing about existence,
content, the writing app or authorship. `kind`, `envelope_author` and `hydrated_at` stay `null`
unless an owner-approved hydration (4.1) has read the envelope. There is no title, content or
deletion field.

## 8. Decisions and remaining review items

Decided: no namespace watch (1); Resource graph model (4); top-level string kind scoped by
namespace (3.1); every current following, reach and web-of-trust behaviour kept exactly, no new
relation filter (2.5); target freshness by consumers' scheduled refresh, eventual and without an
SLA (2.4).

Not authorised by this design:

- **Referenced-target hydration (4.1)** is an unapproved, future, owner-scoped implementation:
  the first time Nexus would read a target. It needs the owner's explicit approval of the read
  itself and its bounds before any implementation; review of this document does not grant it.
  Until then `kind` and `envelope_author` are never set.

Open for the reviews of later implementation changes:

1. Whether the address-derived `namespace` and the cursor paging (7.4) land on their own.
2. Whether Nexus should serve the reach-set route Pinakes asks for.
3. The spec work (3.3): a released spec carrying the 1.1 draft's namespace rule and reference
   parser, and a namespace-agnostic indexer kind type with a kind-name syntax, which no pin has.
