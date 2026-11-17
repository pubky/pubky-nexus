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

`GET /v1/resource/{resource_id}/replies` and `/embeds` return live `/v1` post views,
newest first with author/id ties, `skip` up to 10,000 and `limit` 1–100 (default 20).
They accept the `/v1` post-view options (viewer, tag and tagger limits; `/v1` has no
attachment metadata). Existing Resources with no matches return `200 []`; a missing
Resource returns 404 and malformed parameters return 400. The `/v1` resource views and
tag responses include `replies_count`, `embeds_count`, and `collections_count`. `/v0` is
frozen: it serves neither route (404) and its resource views and tag responses keep
their shape, with no counts. On `/v1`, post relationships carry the external reply URI
in `replied` and an external embed URI in `embedded`. The cache keeps both, but `/v0`
serves its frozen shape: no `embedded`, and `replied` only when it names a post, `null`
for an external reply, as before these references.

Resource replies stay in the author's replies rather than root timelines, including
after cache loss. Deleting the last incoming reference removes the Resource; a
remaining tag, reply, embed, collection or another relationship keeps it.

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
No new graph index or destructive migration is needed. Existing data is populated
on replay of source post PUTs; the resource-feed seed job rebuilds the feed from
already indexed edges. The feature builds on social v1 indexing and the resource
changes feed.

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
  author's replies.
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
