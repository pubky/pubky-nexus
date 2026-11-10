# External post references

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
remaining tag, reply, embed, collection or another relationship keeps it. A deleted
post's tombstone does not retain external-reference counts.

The resource changes feed emits the reserved `reply`, `embed`, and `collection`
kinds. See [the delivery and replay contract](resource-changes.md#post-references).
No new graph index or destructive migration is needed. Existing data is populated
on replay of source post PUTs; the resource-feed seed job rebuilds the feed from
already indexed edges. The feature builds on social v1 indexing and the resource
changes feed.
