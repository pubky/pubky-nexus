# Social v1 indexing

Nexus indexes public `social/v1` profiles, stored post versions, follows and
tags through the pinned social specs translator. Collection posts use the same
post handler and materialize their collected-post edges. Private paths, bookmarks,
files, feeds, mutes, unknown resources and unsupported epochs remain skipped.
Legacy `pubky.app` indexing remains available. This enables public v1 indexing; it does not
flip every user's epoch or backfill historical homeserver data.

Post versions live at `/pub/social/v1/posts/{id}/{editId}[-{slug}].json`. The
translator stores one node under the versionless address
`pubky://{author}/pub/social/v1/posts/{id}`. Each accepted PUT updates that node in
processing order; Nexus does not select a head by comparing edit IDs. A DEL of
**any version** deletes the logical post (or clears it to the existing social
post tombstone when relationships require it). Further version DELs are harmless.
Deleting an old version is therefore not a way to prune history while retaining
the current indexed post.

Social v1 tags on posts and profiles use their existing handlers. Tags on external
URLs, files and foreign app objects use Resource nodes. Tags cannot target another
tag. The existing Resource writer uses `social` as its internal tag scope.
Legacy app-folder tags retain the base branch's behavior;
upstream #1165 owns their removal and the broader tag-scope changes.

Moderation checks both the event address and the existing canonical takedown key,
so writing another social post version cannot bypass a recorded post takedown.
