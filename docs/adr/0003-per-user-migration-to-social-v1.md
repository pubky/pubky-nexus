# ADR 0003: Per-User Migration from `pubky.app` to `social/v1`

## Status

Proposed — 2026-11-17

## Context

The app data model moves from `pubky-app-specs` (v0, `/pub/pubky.app/…`) to
`pubky-social-specs` (v1, `/{pub|priv}/social/v1/…`). Users move one at a time, so for
a long period the two layouts coexist and reference each other:

1. The app's migrator, shipped with a forced update of pubky.app, copies a user's
   objects to `social/v1`.
2. Its last write is `social/v1/profile.json`. The migration flag is private, so this
   public profile is the only signal Nexus gets that the copy is complete.
3. A cleanup pass later deletes the `pubky.app` copies.

The constraints:

- Both layouts arrive on the same event feeds. The migrator's copies arrive as ordinary
  `social/v1` PUTs, and the cleanup as `pubky.app` DELs.
- A migrating user's posts, counts, tags, follows, files and notifications are already
  indexed under v0 addresses, and other users' replies and tags point at them.
- A tag id is `blake3("<target address>:<label>")`, so a tag's id changes when its
  target's address does. A v1 file is `{hash}.{ext}` with no metadata object, where a v0
  file is a `{tsid}` with a `File` node.
- Bookmarks and mutes move to `/priv/social/v1/`, which the event feeds don't carry.
- `/v0` clients (pubky.app, loopky, pubky-rooms, mapky) can't all move at once.

## Decision

Each user carries an **epoch** on the `User` node: `v0` or `v1`, with a missing value
read as `v0`. Nexus reads each user through exactly one layout. The user's own
`social/v1/profile.json` PUT **flips** them. At the flip, Nexus rewrites what it already
holds for them to their v1 addresses (the **internal migration**). It never ingests the
migrator's copies.

### Invariants

1. A user is read through exactly one layout, chosen by their epoch, and the epoch only
   moves up.
2. At `v0`, a `pubky.app` event gives the same graph, Redis and API result as before
   this change.
3. An ignored event is a skip: no fetch, no stored event line, no retry, and the cursor
   advances.
4. A user moves to `v1` only after their internal migration has run, and every flip
   step converges when it runs again.
5. Every address is judged by its own epoch's parser, with no fallback between them.
   Handlers and `nexus-common` take no spec type and parse no address.
6. Content is stored exactly as the author wrote it. The migrator's output is never
   stored.

### Pipeline

```text
route     events/route.rs     namespace → Legacy (pubky.app) | Social (social/v1) | UnsupportedEpoch | Malformed
classify  events/classify.rs  index, or skip before any fetch (counted on watcher.epoch.skipped)
gate      events/gate.rs      the user's epoch, read from the graph: handle | flip | ignore | defer
flip      events/v0_to_v1/    only a v0 user's social/v1 profile PUT, before any fetch
fetch     PUT only
translate translation/{legacy_v0,social_v1}.rs, pure: the epoch's reader validates, fields map
          into Nexus-owned inputs (PostInput, UserInput, TagInput, …), every address inside
          resolves once to Reference { uri, target }
handle    one handler per operation, both epochs
```

`pubky-social-specs` replaces `pubky-app-specs`. Its frozen `legacy_v0` module reads
v0, and its v1 readers come from the same crate. `PubkyId` is shared by both epochs and
must be canonical (ending in `y` or `o`). A translation failure is never retried:
`InvalidEventLine` for a resource no translator handles, `SpecValidation` for a payload
the reader rejects. A reference to a target Nexus doesn't model is `UnsupportedTarget`
and is dropped. The stored models own `PostKind`, `UserLink`, and plain `attachments`
and `links` lists. v1 fields with no slot in the inputs are dropped: attachment names,
the article cover, the version history. A later `social/v2` adds one translator and one
arm in the router.

### The gate

| User | `pubky.app` event | `social/v1/profile.json` PUT | Other `social/v1` event |
|---|---|---|---|
| `v0` (or no node) | handled. Deferred if it's a DEL and the user's flip is queued | **flip** | ignored (the migrator's copies). Deferred if the user's flip is queued |
| `v1` | ignored (cleanup pass, delayed events, an old device) | handled as a profile edit | handled |

The gate reads `coalesce(u.epoch, 'v0')` from the graph on every event, with no cache.
The API reads it from the graph too, with the user's details. Ignored events count as `reason=gated` and
deferred ones as `reason=deferred`.

### The flip

On a `v0` user's `social/v1/profile.json` PUT, before any fetch:

1. **Internal migration**, in batches of 100, each batch its own transaction, with no
   `SKIP`:
   - **Files:** copy each legacy blob's `main` to
     `files_path/v1/<owner>/{hash}.{ext}/main`. The copy checks the bytes against the
     hash, writes a temp file, syncs it and renames it. A source that fails the check or
     can't be read is logged at `error` and skipped. Legacy folders and `File` nodes
     stay.
   - **Posts:** rewrite `Post.uri` and `attachments` to v1 addresses, with the same
     function the specs crate's migrator uses. Rewrite `Post:Details` and rebuild the
     `Post:Relationships` of replies and reposts of the user's posts. Redis entries are
     rewritten, not cleared, because content search, tag search and `sync_del` read
     those keys without refilling them.
   - **Tags the user wrote** under `pubky.app/tags/`: `t.uri` becomes the v1 tag file,
     and `t.id` becomes the v1 id when the target is a post, a profile (`profile.json`
     spelling) or the user's own file. Other targets keep their id, as the migrator
     keeps those targets.
2. **Write `epoch = 'v1'`.** This is a `MERGE`, so it creates a stub when the user has
   no node yet.
3. **Fetch the profile** and index it as the ordinary `v1` profile PUT it now is.

The profile image is never fetched at the flip, so no homeserver request can decide a
flip. A test asserts that the migration's addresses and tag ids match the specs crate's
`migrate::transform` (`v0_to_v1/migrator_tests.rs`). The `migrator` feature is a
dev-dependency only.

**A flip is never lost.** Any failure in steps 1–2 is a single error, `FlipFailed`:
always queued with the transient backoff, never a batch stop, and with no attempt limit
while the user is `v0`. Its retry reruns the whole flip over any partial work. While the
flip waits in the retry queue, the gate holds that user's `pubky.app` DELs and their
other `social/v1` writes as `MissingDependency` on the flip. They run once the user is
`v1`, and each PUT fetches the current resource, so the last state wins. The rules
behind this are pure functions with table tests: `waits_for_flip`, `is_queued_flip` and
`retry_outcome`.

### Identity across the flip

| Object | Rule |
|---|---|
| Post, profile, follow | One node for both copies, keyed by owner and id. Replies and tags that use either epoch's address link to the same node |
| v1 post versions | Every version PUT takes the edit path, and any version DEL deletes the post. The edit id isn't kept (see E1 under Consequences) |
| Tag edges | One edge per tagger, target and label. The folder's `app` is no longer stored. Other users' tags on a migrated user's posts keep their id, and the frontend untags at `tag_uri` |
| Tag folders | Only `pubky.app/tags/` and `social/v1/tags/` are read. `/pub/<app>/tags/` is skipped, PUT and DEL alike, and edges indexed from there before stay. The target decides the tag's flow: a post or user gets a post or user tag, a URL, another app's object or a file gets a `Resource` tag, and anything else is rejected |
| Files | A v1 file is bytes with no node. `/v1/static/files/{owner}/{id}/{variant}` serves both `{tsid}` and `{hash}.{ext}` |
| Avatar | Mounted at `/static/avatar` and `/v1/static/avatar`. Answers 404 for anything that isn't an image Nexus holds |
| File serving | Every file response carries `nosniff`, and only passive media types display inline |
| Takedowns | A v1 post version is keyed by its versionless post address. A file is keyed by its blob, so the takedown holds under any extension and against the flip's copy |
| Collections | Items are read through the v1 envelope first, then the legacy one, each item address through either parser |

### API

Every route is declared once and mounted under `/v0` and `/v1`, with one OpenAPI document
per version. The difference between versions is a type parameter (the `ApiVersion`
trait), with thin per-version wrappers for utoipa. `/v1` differs from `/v0` in these
ways:
- Post kinds are named the v1 way: `note`, `article`, …, `unknown`.
- `kind` and `exclude_kinds` parse strictly against the v1 names and answer 400 for
  anything else.
- There are no bookmarks: Nexus never sees v1 bookmarks, which are private.
- There are no file details.
- Tags have no `relationship` field, and tag labels fold by the v1 rule.

`/v0` stays byte-for-byte as it was, takes fixes only, and is removed once every client
has left it.

### What Nexus requires from others

| Who | Requirement |
|---|---|
| Deploy | N3, N4, N5 and API1 are live before any client writes `social/v1`. Otherwise those events are lost and need a re-index |
| App migrator | Writes `profile.json` last |
| App cleanup pass | Deletes the `pubky.app` copies only once Nexus reports `epoch: v1` |
| App signup (A7) | Writes `profile.json` before uploading the avatar |
| Frontend | Untags at `tag_uri` before the first flip |
| Moderation | Its key never migrates. It tags a post again when it comes back at its `social/v1` address (to confirm with that service) |

## Consequences

### Positive ✅

- A migrating user keeps every post, count, tag, follow, file and notification. Only
  their own addresses change, and the copies send no "post edited" notifications.
- Users migrate on their own clock, with no coordinated cutover and no re-index.
- A homeserver outage or graph error during a flip delays it instead of losing it, and
  the user's writes in the meantime are held rather than dropped.
- Handlers no longer depend on a spec crate, so the next epoch only adds a translator.

### Negative ❌

- **One-way door for data.** After a user flips, their posts, tag edges and files carry
  `social/v1` addresses in the graph and under `files_path/v1/`, which a v0-only binary
  can't read. Before any flip, the change can be rolled back.
- **Breaking changes:**
  - A non-canonical 52-character id in an API path answers 400 instead of 404.
  - `?app=` on `/stream/resources` and `/stream/resources/ids` answers 400.
  - Tags in `/pub/<app>/tags/` are no longer indexed. That affects loopky, pubky-rooms,
    mapky.app, vibes and eventky.app until they write to `social/v1/tags/`.
  - Homeserver lookup failures now fail closed.
- **Cost to run:**
  - Up to 8.1 GB more disk until the legacy folders are deleted.
  - Each flip runs inline, with no time limit, and holds up the events behind it. The
    largest hosted user has 6,396 posts, 6,255 tags and 765 files.
  - File variants are rebuilt on first request, so expect a surge after the forced
    update.
  - The `pubky` 0.10 bump was reverted once (#1048) after pkarr resolve failures in
    production, with the cause unknown.
- **Accepted limits:**
  - A held write dead-letters after the dependency limit (50 attempts, about two days).
    A flip pinned by a bug in a migration query retries until a release fixes it, and
    the user's content is still served at `v0` meanwhile.
  - A migrator copy that reaches Nexus after the flip is taken for an edit: from a
    replayed flip batch, from behind a failed flip, or from a resumed migrator run. A
    long post's content is then replaced, and its repliers are told it was edited.
  - A `pubky.app` takedown doesn't reach the post's `social/v1` versions.
  - A `pubky.app` PUT waiting in the retry queue across the flip is lost.
  - Objects the migrator skips can't be edited or removed afterwards (24 hosted tags).
  - A new user's writes before their first profile are ignored.
  - A hard-deleted `v1` user reads as `v0` again.
  - After an author's v1 DEL of a file, the file stays public at its `{tsid}` address.

### Neutral ⚠️

- URIs are stored as received, so a cross-epoch reply, tag or notification names its
  target the way its author spelled it. Notifications stored before a flip keep the old
  addresses.
- The 836 resource tag edges that carry `app` keep it, and nothing reads it.
- `watcher.epoch.skipped{reason=deferred}` counts every hold, retries included. Its rate
  shows how often events are held, not how many are held.
- The URI backfill reads the event log with the frozen v0 reader, so it doesn't depend
  on the live router.
- **Still open:**
  - **E1:** the newest version wins and a DEL of a single version is handled. Due before
    the pubky.app release (A6).
  - **F2:** a per-app resource stream to replace `?app=`.
  - A `nexusd` job that deletes the legacy file folders and `File` nodes.
  - The `/v0` removal date and R1, the bookmark cleanup. Both wait on knowing which
    clients still read `/v0`.

## Alternatives Considered

| Option | Why not |
|---|---|
| Switch to `social/v1` only | Stops indexing users who haven't migrated and breaks cross-epoch references |
| A second watcher for the legacy layout | Same graph, so copies still collide. It also loses per-homeserver ordering, adds a second cursor family, downloads `/events/` twice, and resolver, retry and jobs must still run in one instance |
| Ingest both epochs for every user | Every migrated object would be fetched and handled again, all at once when the forced update lands. Handlers would call homeservers, and it needs tag id sets and a global file re-key |
| Translate v1 into the `legacy_v0` types | Ties every handler to the frozen crate and makes v0 the model for good |
| Two spec crates side by side | Two `PubkyId`s. Upgrading `pubky-app-specs` past 0.7 required `pubky` 0.10 in any case |
| Cache the epoch in Redis (`User:Epoch`) or in process | A concurrent `pubky.app` profile PUT can cache a stale `v0`, and the user's first v1 events would then be dropped |
| Set `v1` before the migration, with a completion marker | A retry would see a `v1` user and skip the migration. Setting `v1` last makes the retry rerun the whole flip |
| Flip after the profile fetch (N3's first shape) | A homeserver outage could dead-letter the flip. Running the flip first leaves the one step Nexus doesn't control for last |
| Internal migration as a queued job | A v1 event could overtake it |
| Clear the Redis entries instead of rewriting them | Search and `sync_del` read those keys without refilling them |
| Move the file bytes | Uploads of one blob share a folder, and the epoch can't tell a reader which folder holds a file |
| Hard-link the file bytes | A legacy write would change the v1 file, so it would no longer match its hash |
| Delete legacy `File` nodes at the flip | Can't be undone, and the flip's retry needs them. No hosted post or profile names another user's file |
| A set of tag ids per edge | Only needed if both epochs are ingested. One layout per user needs one id |
| A fixed `app` (`social`) on v1 resource tags | A tag is one statement whatever app wrote it, and the tag id already ignores the app |
| Store collection items resolved on the node | A migration for one function. Reading them at read time works for both envelopes |
| Look a post takedown up under both epochs' addresses | The moderation service tags a post again at its new address, and that takedown then holds across all of the post's versions |
| A runtime API version value | The v1 OpenAPI document couldn't be generated from the types that serialize |
| Keep a user's epoch after a hard delete | It would make the epoch durable Redis state. The app writes nothing for a user without a profile |
