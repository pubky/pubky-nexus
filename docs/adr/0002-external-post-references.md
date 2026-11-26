# ADR 0002: External Post References

## Status

Proposed — 2026-10-27

## Context

Social v1 lets a post reply to, embed or collect any URI, not only another post.
Before this, a reply to a URL or an external embed was indexed with no edge to its
target: a consumer could not tell which posts referenced a Resource, there were no
counts, and a tag and a post naming the same URI did not meet.

## Decision

A post's references to external targets become edges to the same `Resource` node that
tags use, and their changes go on the resource changes feed
([ADR 0001](0001-resource-changes-feed.md)). Nexus does not fetch the targets.

A social or legacy post replying to an external URI creates a `REPLIED` edge to
its Resource. External embeds create `EMBEDS`; external collection items create
`COLLECTED`. Native post replies, reposts and collected posts keep their existing
paths. Known profile/file/tag reply rejections remain unchanged. A post reference
makes a Resource edge only to an external target: another scheme's URL or another
app's public object. A post reference to a profile, file or tag (as an embed or
collection item), or to a bookmark, a follow, a private path or anything else Nexus
never references, makes no Resource edge, feed line or count; a Resource that
exists for another reason, such as a tag on a file, is left as it is. A reply to
such a target is a root post in its counts, timelines and engagement ranking. All
Resource targets use the tag normalizer and ID, so tags and posts share one node.

A post's reply parent is fixed, whatever it names: an edit may not add, drop or change
it. A post parent is compared by its key, an external parent by its normalized Resource
id, so another spelling of the same URL is no change; an edit that changes either is
refused and never retried. Embeds and collection items stay editable. The rule holds
once the post's references were reconciled: a post's first reconcile writes its
reference revision, also with no reference. A post without a reference revision has no
Resource reply edge yet, and its next PUT gains the one it names.

Each PUT diffs the post's references against its edges and publishes the change: the
dels of the references it loses and the puts of the ones it gains. A reference is keyed
by the post's address, not by the version file that names it, so a PUT that names the
same targets writes and publishes nothing, whichever version file it is, and a repeated
event line does not grow the feed. A delete marks the post's references as
retracted when it appends their dels, so the next PUT (after a delete that stopped
half-way, or one that brings a tombstone back) puts every current reference again. An
edge keeps the time it was first indexed. A reference write whose post changed under it
(a newer version, a delete) writes nothing and fails with a typed stale-write error,
which the watcher retries.

`GET /v1/resource/{resource_id}/replies` and `/embeds` return live `/v1` post views,
newest first with author/id ties, `skip` up to 10,000 and `limit` 1–100 (default 20).
They accept the `/v1` post-view options (viewer, tag and tagger limits; `/v1` has no
attachment metadata). Existing Resources with no matches return `200 []`; a missing
Resource returns 404 and malformed parameters return 400. The `/v1` resource views and
tag responses include `replies_count`, `embeds_count`, and `collections_count`: they count
live posts, not authors. Collections are counted and published on the feed but not
listed by a route. The views count the live posts that reference the Resource only
on `/v1`; `/v0` views and streams skip those counts. `/v0` is frozen: it serves
neither route (404) and its resource views and tag responses keep their shape, with
no counts. On `/v1`, post relationships carry the external reply URI in `replied`
and an external embed URI in `embedded`. `/v0` serves typed post shapes built from
the stored models: a post's relationships there have no `embedded`, and `replied`
names only a post (`null` for an external reply). Whether the reply parent is a post
comes from the graph and is cached with the relationships, never read from the
address. For posts without external references, `/v0` answers are the stored models'
bytes: the same fields in the same order.

Two `/v0` changes are part of the feature. A URL that only posts reference is a
Resource, so `GET /v0/resource/{id}/tags` and `GET /v0/resource/by-uri` answer 200 with
`tags: []` for it. A reply to a URL is a reply on `/v0` too: it stays out of
the root timelines and counts in the author's replies.

Resource replies stay in the author's replies rather than root timelines, including
after cache loss, and from a new reply's first index write, before its Resource reply
edge exists. After the reference write the handler reads the post's cached
relationships and lists back from the graph when the post has or had a Resource edge,
or when its lists disagree with what it is, and ranks a root post that is not ranked:
a retry after a failure past the reference write finds the same facts. A delete always
takes the post out of the engagement ranking and the author's replies. Deleting the
last incoming reference removes the Resource; a remaining tag, reply, embed, collection
or another relationship keeps it.

A deleted post that still has other relationships (replies, reposts, tags and so on)
becomes a tombstone, which keeps its Resource edges. Every Resource-side reader skips a
deleted post: the Resource's reply and embed lists, its counts and the feed's seed. A
tombstoned external reply is still a reply, through its kept `REPLIED` edge: it stays
out of root timelines and in the author's replies and reply count. Its references are
retracted on the feed when it becomes a tombstone; its later hard delete removes the
edges without retracting them again, and a Resource it leaves with no relationship
goes with them.

The resource changes feed emits the reserved `reply`, `embed`, and `collection`
kinds. See [the delivery and replay contract](0001-resource-changes-feed.md#post-references).
The seed writes the post reference lines after the tag lines, in edge time order.

A post's handlers share one lock per post (author and post id; the stripe is picked
with a hasher seeded once per process, so nobody can pick a post id that shares
another's). A PUT looks up the unknown authors of its collection items after it releases
the lock, and only when it wrote a new post or changed one: an unchanged replay, a
refused edit and a PUT that waits for a missing dependency look nobody up. A
`social/v1` post DEL that finds no post (one never indexed, or one another
version file's DEL already deleted) is acknowledged without storing its event line, as
for a `pubky.app` post.

No new graph index or destructive migration is needed. Existing data is populated
on replay of source post PUTs; the resource-feed seed job rebuilds the feed from
already indexed edges. The feature builds on social v1 indexing and the resource
changes feed. A post's outbox that no reader can parse (a lost append in a format a
later release cannot read) is logged at error level and cleared, never retried, so it
blocks none of the post's handlers. The feed lines it held may then be missing: running
the `resource-changes-seed` job repairs the feed.

## Consequences

### Positive ✅

- Tags and posts that name the same URI share one Resource and one feed.
- `/v1` serves the posts that reply to or embed a Resource, and reference counts on
  every Resource view.
- No new graph index or destructive migration.

### Negative ❌

- No backfill: posts indexed before this change gain their references only when their
  PUT is replayed. The seed job rebuilds the feed from edges that exist.
- Feed consumers key post references by `(author, object_path, kind, resource_id)`,
  and tags by `(author, uri, label)`.

### Neutral ⚠️

- An external reply counts as a reply: it stays out of root timelines and in the
  author's replies, on `/v0` too.
- A post's reply parent, a post or an external target, is fixed from its first
  reconciled PUT on.
- A Resource lives while any reference to it remains.

## Alternatives Considered

### No edge for external references

The state before this change: consumers cannot find the posts that reference a
Resource, and the counts are missing.

### Resources for every post reference

A profile, file or tag is a native object, and a bookmark, a follow or a private path
is never referenced by Nexus. Resource edges for them would duplicate native
relationships and count objects Nexus does not model, so only external targets get one.

### Removing a tombstone's Resource edges

A tombstone that dropped its Resource edges would need a flag to stay a reply, and
every reader that tests a post's reply edge (counts, timelines, the author's replies,
the ranking refresh) would need to read it too. Every Resource-side reader already
skips a deleted post, so the tombstone keeps its edges as it keeps all others.

### A separate node per reference kind

Tags and posts on the same URI would not meet. One Resource per normalised URI keeps
one identity for everything that references it.
