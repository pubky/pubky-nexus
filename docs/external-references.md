# External post references

A social or legacy post replying to an external URI creates a `REPLIED` edge to
its Resource. External embeds create `EMBEDS`; external collection items create
`COLLECTED`. Native post replies, reposts and collected posts keep their existing
paths. Known profile/file/tag reply rejections remain unchanged. All Resource
targets use the tag normalizer and ID, so tags and posts share one node.

`GET /v0/resource/{resource_id}/replies` and `/embeds` return live post views,
newest first with author/id ties, `skip` up to 10,000 and `limit` 1–100 (default 20).
They accept the ordinary post-view options, including viewer and attachment
metadata. Existing Resources with no matches return `200 []`; a missing Resource
returns 404 and malformed parameters return 400. Resource views and tag responses
include `replies_count`, `embeds_count`, and `collections_count`. Post relationships
carry the external reply URI in `replied` and an external embed URI in `embedded`.

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
