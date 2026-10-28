# Social events, RSVPs, ratings and availability

Nexus indexes public social `event`, `rsvp`, `rating` and `availability` posts itself. Files use
`/pub/social/v1/posts/{id}/{editId}.json`; logical identity is the versionless URI.
The social 1.1 draft validator is pinned to
[`b2916840c610cece6174f96a4d364df600988d73`](https://github.com/pubky/pubky-social-specs/commit/b2916840c610cece6174f96a4d364df600988d73)
until a release is available. Existing v0 and v1 post spellings remain supported.
Private documents are not indexed. These projections add no resource-feed lines.
Ordinary universal reply/embed references retain their existing feed behavior.

## Calendar projection

Events store `start_at` and `end_at` as UTC milliseconds, plus `all_day`, `status`,
`policy`, `capacity` and `waitlist`. Location URIs use the common Resource
normalizer and ID function; each is connected by `LOCATED_AT`. Nexus performs no
geocoding or spatial filtering. Event title and description are searchable via
`GET /v0/search/posts/by_content?q=...&kind=event`.

Time conversion uses the IANA database bundled with chrono-tz. Following
[RFC 8984 sections 1.4.5–1.4.6](https://www.rfc-editor.org/rfc/rfc8984.html#section-1.4.5),
clock gaps and folds use the offset before the transition. Duration days/weeks
advance the local date; hours/minutes/seconds advance elapsed time. Consequently,
`P1D` and `PT24H` can differ across daylight-saving transitions. An all-day event
without duration defaults to one local calendar day.

At write time, Nexus expands the draft's daily, weekly, monthly and yearly rules,
including interval, week start, month/day filters, ordinal weekdays, set positions,
count/until and excluded dates. All rules contribute to one sorted, deduplicated set.
The pinned wire validator currently accepts at most one rule; the expansion engine
also handles multiple rules when supplied by a future validator or translator.
Ordinal weekdays require monthly/yearly frequency.
The lifetime is bounded to 1,000 occurrences and three calendar years from DTSTART;
a start more than three years in the future is rejected. Invalid month days are
omitted; exclusions apply after count. Only ongoing/upcoming instances whose start
is before twelve months ahead are materialized. Past occurrences are not an archive.

Each occurrence is keyed by `(master URI, recurrence_id)`, where `recurrence_id` is
the original local start. `post_uri` identifies the master or overriding post.
Same-author overrides replace the whole instance; the newest original `indexed_at`
wins (post ID breaks ties). Deleting an override restores the next surviving override
or generated instance. An override whose key no longer exists becomes an ordinary
event under its own URI. Source snapshots and a graph lock prevent a racing rebuild
from replacing newer input with stale results.

The `event-occurrences` job rolls the window daily at 04:20 UTC. It also drains
pending change notifications. `orphan-rsvps` runs Sundays at 04:40 UTC, removing the
attendance projection for missing recurrence keys while preserving the source reply.
The complete bounded recurrence set is used for cleanup, so a valid instance outside
the twelve-month window is not an orphan. Run either job manually with
`nexusd jobs run <name>`; configure schedules under `[jobs.<name>]`.
Replies to an override resolve its master series before checking recurrence keys.
A missing series projection is left alone while its parent write/rebuild can retry.

## Attendance and ratings

An RSVP must target an indexed, undeleted event. A missing parent is retried as a
dependency; an indexed non-event parent is rejected. One effective RSVP is selected
per author and event/occurrence. The newest indexed content wins across duplicate
RSVP posts; an unchanged retry does not change that order. An occurrence-specific
answer, including a decline, supersedes a series answer. A reply directly to the
active override also applies to that occurrence.

Counts separate requested, accepted, declined and tentative. `going` is the accepted
headcount plus guests; ticket policy ignores guests. `requested` never counts as
`going`. Attendees are ordered by their winning post's original `indexed_at`, then
author. When waitlisting is enabled, accepted parties exceeding cumulative capacity
are marked `waitlisted`; this count is included in `going`, not subtracted from it.
This is public attendance information, not an admission or ticket entitlement check.

Ratings accept integers **1–10** and require a parent URI. The newest indexed rating
per author/parent wins; edits and deletes recompute the target's `rating_avg` and
`rating_count`, with zero/zero for no ratings. Both Post and Resource views expose
these fields. A missing social or legacy pubky post target is retried as a dependency;
its rating never falls back to a Resource node. Other URI targets use Resource nodes.
Native parent validation and rating updates use the resolved author/post ID, so v0/v1
aliases and later stored-URI changes cannot strand an aggregate. RSVP projections
store the indexed event's current URI. The pinned wire validator requires a social/v1
RSVP parent; the handler also resolves legacy aliases when supplied by a translator.
Resource views read their own aggregate by Resource ID. External-URI ratings also
create the ordinary external reply edge, count and resource-feed line.
Existing post/resource streams support `sorting=rating`; score cursors
can be fractional. Other sorting modes keep integer timestamp/count JSON values.

RSVPs and ratings retain ordinary reply notifications. `event_changed` is sent to
accepted attendees when an indexed occurrence's start, normalized location set or
status changes, or a future occurrence is removed by an edit/delete. The notification
contains `event_uri`, `recurrence_id` and `post_uri`. Occurrence-specific declines
suppress a series acceptance. Notification records are persisted with the occurrence
write and retried idempotently after a Redis failure; a graph commit followed by a
failed notification append does not lose the notification. Natural window aging does
not emit change notifications. Events beyond the materialized window have no
occurrence-change notifications until they enter it.

## Filters and bounded event views

`GET /v0/stream/events` combines `from`/`to`, `author`, `authors` (1–100 comma-separated
public keys), `tags`, `status` (`confirmed`, `tentative`, `cancelled`), `collection`,
`source=following&user_id=...`, `going=<user>` and `locations`. Filters intersect;
multiple authors or locations match any entry. Pagination is skip 0–10000 and
limit 1–200, default 20, in start/master/recurrence order.

`locations` accepts 1–100 Resource IDs or universal URIs, normalized with the same
functions as tags, and matches the effective event's `LOCATED_AT` edges. It can
use place IDs returned by a spatial resolver. CSV-quote URIs containing commas
and URL-encode the whole parameter, for example:

```sh
curl --get "$NEXUS/v0/stream/events" --data-urlencode 'locations="geo:47.2,8.3",https://example.com/venue'
```

`going` includes accepted and tentative answers. The newest occurrence-specific
RSVP overrides the series answer, including a decline. RSVP parents **must be
Pubky event posts**; an external event URI or a plain post is rejected before
any graph projection or reference append. This filter does not infer attendance
from external Resources. Location-filtered queries start from indexed Resource IDs,
and going-only queries start from that user’s RSVP posts. Occurrences have separate
master and post_uri indexes; event and collection lookups use indexed UNION matches.

`GET /v0/event/{author}/{id}` returns ordinary post details, tags and taggers,
counts, effective attendees and upcoming occurrences. `viewer_id` preserves tag
relationship data. `limit_tags` and `limit_taggers` are 1–100 (default 10 each),
`limit_attendees` is 1–200 (default 20), and `limit_occurrences` is 1–50 (default 50).
These limits never truncate counts. `/attendees?status=` accepts requested,
accepted, declined or tentative; pagination remains independent of event-view limits.

Requests read precomputed occurrences and never expand rules. Attendees are
filtered, paged and counted in the graph: a row's waitlist position only needs the
accepted parties before it, so a page never loads the whole RSVP set. Deleted
profiles are neither listed nor counted. Rating fields are read from graph aggregates so cached post
bodies cannot hide an updated score. Historic posts previously indexed as unknown
need their source versions replayed through the watcher to gain native projections.

## Calendars

A calendar is an ordinary collection; Nexus has no calendar endpoints. List
collections that contain events with the post stream:
`GET /v0/stream/posts?kind=collection&item_kind=event` (any `source`, sorting
and pagination of that stream; `/v0/stream/posts/keys` accepts the same filter).
`item_kind` requires `kind=collection` and keeps collections that list at least
one live indexed post of that kind. The kind index selects collections and each
one expands only its own COLLECTED edges. External items are not counted.

A calendar's upcoming events come from
`GET /v0/stream/events?collection=<versionless collection URI>`: indexed
occurrences of the collection's items (series and overrides) in start order,
with the usual time window, filters and paging. The query starts from the
collection's items. External event items remain links; their times are not
inferred or fetched.

## Availability

`GET /v0/availability/{author}` serves current schedules by author.
`GET /v0/stream/availability?for=<URI>` serves them by normalized target; author
and for can be combined. The stream requires at least one. Both return one live
schedule per `(author, normalized for)`, newest accepted content first, with post
ID as a deterministic tie breaker. A post without for names `pubky://<author>`.
Replay of unchanged content keeps its timestamp and position. Deletion or a
change of kind/target reveals the next newest surviving schedule.

The shared 1.1 validator checks time zones, horizons, available/busy blocks and
references. Nexus stores the validated content and normalized target, not expanded
slots or booking rights. Available blocks retain capacities; busy blocks have no
title or capacity. Pagination is skip 0–10000, limit 1–200, default 20. No app
namespace receives special treatment.

## Known follow-ups

Global rating sorts still scan Posts/Resources and need maintained sorted sets.
Rating cursors carry only the score, like the other streams; ties resume with
`skip`. Overrides of one series have no count bound of their own.
