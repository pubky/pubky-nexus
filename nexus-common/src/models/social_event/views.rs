use super::Occurrence;
use crate::db::{fetch_all_rows_from_graph, fetch_row_from_graph, graph::Query};
use crate::models::{
    error::{ModelError, ModelResult},
    post::PostView,
};
use crate::universal_tag::normalize::resource_id;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct RatingSummary {
    pub rating_avg: f64,
    pub rating_count: i64,
}
pub async fn rating_summary(uri: &str) -> ModelResult<RatingSummary> {
    let row = fetch_row_from_graph(
        Query::new(
            "rating_summary",
            r"
        CALL () {
            MATCH (p:Post {uri:$uri}) RETURN p, 0 AS priority
            UNION ALL
            MATCH (p:Resource {id:$resource_id}) RETURN p, 1 AS priority
        }
        RETURN coalesce(p.rating_avg,0.0) AS average,coalesce(p.rating_count,0) AS total
        ORDER BY priority LIMIT 1
    ",
        )
        .param("uri", uri)
        .param("resource_id", resource_id(uri)),
    )
    .await?;
    match row {
        Some(r) => Ok(RatingSummary {
            rating_avg: r.get("average")?,
            rating_count: r.get("total")?,
        }),
        None => Ok(RatingSummary::default()),
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct EventCounts {
    pub requested: i64,
    pub accepted: i64,
    pub declined: i64,
    pub tentative: i64,
    /// Accepted attendees plus guests; guests are ignored for ticket policy.
    pub going: i64,
    pub waitlisted: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct Attendee {
    pub author: String,
    pub post_uri: String,
    pub status: String,
    pub recurrence_id: Option<String>,
    pub guests: i64,
    pub indexed_at: i64,
    pub waitlisted: bool,
}
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct EventView {
    pub post: PostView,
    pub start_at: i64,
    pub end_at: i64,
    pub all_day: bool,
    pub status: String,
    pub policy: String,
    pub capacity: Option<i64>,
    pub counts: EventCounts,
    pub occurrences: Vec<Occurrence>,
    pub attendees: Vec<Attendee>,
}

/// The occurrence an attendance read is about: its master, the effective event post (an
/// override when one replaces the occurrence) and its recurrence id. An override's own URI
/// resolves to its occurrence, so series answers to the master still count for it.
struct AttendanceScope {
    policy: String,
    capacity: Option<i64>,
    waitlist: bool,
    master: String,
    effective: String,
    recurrence: Option<String>,
}

async fn scope(uri: &str, recurrence: Option<&str>) -> ModelResult<Option<AttendanceScope>> {
    let Some(row) = fetch_row_from_graph(
        Query::new(
            "attendance_scope",
            r"
        MATCH (p:Post {uri:$uri}) WHERE p.event_content IS NOT NULL
        OPTIONAL MATCH (by_master:EventOccurrence {master:$uri,recurrence_id:$recurrence})
        OPTIONAL MATCH (by_post:EventOccurrence {post_uri:$uri})
          WHERE by_post.recurrence_id=$recurrence OR ($recurrence IS NULL AND p.event_master IS NOT NULL)
        WITH p, collect(coalesce(by_master,by_post))[0] AS o
        OPTIONAL MATCH (override:Post {uri:o.post_uri})
        WITH p, o, coalesce(override,p) AS effective
        RETURN effective.policy AS policy, effective.capacity AS capacity,
               coalesce(effective.waitlist,false) AS waitlist, coalesce(o.master,$uri) AS master,
               effective.uri AS effective, coalesce(o.recurrence_id,$recurrence) AS recurrence
    ",
        )
        .param("uri", uri)
        .param("recurrence", recurrence),
    )
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(AttendanceScope {
        policy: row.get("policy")?,
        capacity: row.get("capacity")?,
        waitlist: row.get("waitlist")?,
        master: row.get("master")?,
        effective: row.get("effective")?,
        recurrence: row.get("recurrence")?,
    }))
}

/// One effective RSVP `r` per live author `u`: an occurrence answer (or one to the
/// override) beats the series answer, then the newest content wins.
const EFFECTIVE_RSVPS: &str = r"
        MATCH (u:User)-[:AUTHORED]->(r:Post)
        WHERE r.social_kind='rsvp' AND r.rsvp_target IN [$master,$effective]
          AND NOT coalesce(r.deleted,false) AND NOT coalesce(u.deleted,false)
          AND (r.recurrence_id IS NULL OR r.recurrence_id=$recurrence)
        WITH u,r ORDER BY CASE WHEN r.recurrence_id=$recurrence OR (r.rsvp_target=$effective AND $effective<>$master) THEN 1 ELSE 0 END DESC,r.social_updated_at DESC,r.id DESC
        WITH u,collect(r)[0] AS r
";

fn scoped(name: &'static str, cypher: String, scope: &AttendanceScope) -> Query {
    Query::new(name, &cypher)
        .param("master", scope.master.clone())
        .param("effective", scope.effective.clone())
        .param("recurrence", scope.recurrence.clone())
}

fn seats(scope: &AttendanceScope, guests: i64) -> i64 {
    1 + if scope.policy == "ticket" { 0 } else { guests }
}

/// Attendees in FIFO order, filtered and paged in the graph. Whether an accepted party is
/// waitlisted depends only on the accepted parties before it, so reading the first
/// `skip + limit` matching rows is enough.
async fn list(
    scope: &AttendanceScope,
    status: Option<&str>,
    skip: usize,
    limit: usize,
) -> ModelResult<Vec<Attendee>> {
    let (skip, limit) = (skip.min(10_000), limit.min(200));
    let cypher = format!(
        "{EFFECTIVE_RSVPS}
        WITH u,r WHERE $status IS NULL OR r.rsvp_status=$status
        RETURN u.id AS author,r.uri AS post_uri,r.rsvp_status AS status,r.recurrence_id AS recurrence_id,
               coalesce(r.guests,0) AS guests,r.indexed_at AS indexed_at
        ORDER BY r.indexed_at,u.id
        LIMIT $take"
    );
    let rows = fetch_all_rows_from_graph(
        scoped("event_attendees", cypher, scope)
            .param("status", status)
            .param("take", (skip + limit) as i64),
    )
    .await?;
    let mut taken = 0;
    let mut people = Vec::with_capacity(rows.len());
    for row in rows {
        let mut person = Attendee {
            author: row.get("author")?,
            post_uri: row.get("post_uri")?,
            status: row.get("status")?,
            recurrence_id: row.get("recurrence_id")?,
            guests: row.get("guests")?,
            indexed_at: row.get("indexed_at")?,
            waitlisted: false,
        };
        if person.status == "accepted" {
            taken += seats(scope, person.guests);
            person.waitlisted = scope.waitlist && scope.capacity.is_some_and(|cap| taken > cap);
        }
        people.push(person);
    }
    Ok(people.into_iter().skip(skip).collect())
}

/// Counts aggregated in the graph; only accepted parties' guest numbers come back, in FIFO
/// order, to place the waitlist.
async fn counts(scope: &AttendanceScope) -> ModelResult<EventCounts> {
    let cypher = format!(
        "{EFFECTIVE_RSVPS}
        WITH u,r ORDER BY r.indexed_at,u.id
        RETURN count(CASE WHEN r.rsvp_status='requested' THEN 1 END) AS requested,
               count(CASE WHEN r.rsvp_status='declined' THEN 1 END) AS declined,
               count(CASE WHEN r.rsvp_status='tentative' THEN 1 END) AS tentative,
               collect(CASE WHEN r.rsvp_status='accepted' THEN coalesce(r.guests,0) END) AS accepted"
    );
    let Some(row) = fetch_row_from_graph(scoped("event_attendance_counts", cypher, scope)).await?
    else {
        return Ok(EventCounts::default());
    };
    let accepted: Vec<i64> = row.get("accepted")?;
    let mut counts = EventCounts {
        requested: row.get("requested")?,
        declined: row.get("declined")?,
        tentative: row.get("tentative")?,
        accepted: accepted.len() as i64,
        ..EventCounts::default()
    };
    for guests in accepted {
        let size = seats(scope, guests);
        counts.going += size;
        if scope.waitlist && scope.capacity.is_some_and(|cap| counts.going > cap) {
            counts.waitlisted += size;
        }
    }
    Ok(counts)
}

pub async fn attendees(
    uri: &str,
    recurrence: Option<&str>,
    status: Option<&str>,
    skip: usize,
    limit: usize,
) -> ModelResult<Vec<Attendee>> {
    match scope(uri, recurrence).await? {
        Some(scope) => list(&scope, status, skip, limit).await,
        None => Ok(Vec::new()),
    }
}

/// The URI of a live event by a live author, the one the event routes answer for.
pub async fn event_uri(author: &str, id: &str) -> ModelResult<Option<String>> {
    let row = fetch_row_from_graph(
        Query::new(
            "event_uri",
            r"
        MATCH (u:User {id:$author})-[:AUTHORED]->(p:Post {id:$id})
        WHERE p.event_content IS NOT NULL AND NOT coalesce(p.deleted,false)
          AND NOT coalesce(u.deleted,false)
        RETURN p.uri AS uri
    ",
        )
        .param("author", author)
        .param("id", id),
    )
    .await?;
    row.map(|r| r.get("uri").map_err(ModelError::from))
        .transpose()
}

pub async fn event_view(
    author: &str,
    id: &str,
    recurrence: Option<&str>,
) -> ModelResult<Option<EventView>> {
    event_view_with_options(author, id, recurrence, EventViewOptions::default()).await
}

#[derive(Debug)]
pub struct EventViewOptions {
    pub viewer: Option<String>,
    pub tags: usize,
    pub taggers: usize,
    pub attendees: usize,
    pub occurrences: usize,
}
impl Default for EventViewOptions {
    fn default() -> Self {
        Self {
            viewer: None,
            tags: 10,
            taggers: 10,
            attendees: 20,
            occurrences: 50,
        }
    }
}

pub async fn event_view_with_options(
    author: &str,
    id: &str,
    recurrence: Option<&str>,
    options: EventViewOptions,
) -> ModelResult<Option<EventView>> {
    let Some(meta) = fetch_row_from_graph(
        Query::new(
            "event_details",
            r"
        MATCH (u:User {id:$author})-[:AUTHORED]->(p:Post {id:$id})
        WHERE p.event_content IS NOT NULL AND NOT coalesce(p.deleted,false)
          AND NOT coalesce(u.deleted,false)
        RETURN p.uri AS uri,p.start_at AS start,p.end_at AS end,p.all_day AS all_day,
               p.status AS status,p.policy AS policy,p.capacity AS capacity
    ",
        )
        .param("author", author)
        .param("id", id),
    )
    .await?
    else {
        return Ok(None);
    };
    let uri: String = meta.get("uri")?;
    let (people, counts) = match scope(&uri, recurrence).await? {
        Some(scope) => (
            list(&scope, None, 0, options.attendees).await?,
            counts(&scope).await?,
        ),
        None => (Vec::new(), EventCounts::default()),
    };
    let rows=fetch_all_rows_from_graph(Query::new("event_next_occurrences", "CALL () { MATCH (o:EventOccurrence {master:$uri}) USING INDEX o:EventOccurrence(master) RETURN o UNION MATCH (o:EventOccurrence {post_uri:$uri}) USING INDEX o:EventOccurrence(post_uri) RETURN o } WITH o WHERE o.end_at >= $now RETURN properties(o) AS occurrence ORDER BY o.start_at,o.master,o.recurrence_id LIMIT $limit").param("limit",options.occurrences.min(50) as i64).param("uri",uri).param("now",chrono::Utc::now().timestamp_millis())).await?;
    let occurrences = rows
        .iter()
        .map(|r| r.get("occurrence").map_err(ModelError::from))
        .collect::<ModelResult<Vec<_>>>()?;
    let Some(post) = PostView::get_by_id(
        author,
        id,
        options.viewer.as_deref(),
        Some(options.tags.min(100)),
        Some(options.taggers.min(100)),
    )
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(EventView {
        post,
        start_at: meta.get("start")?,
        end_at: meta.get("end")?,
        all_day: meta.get("all_day")?,
        status: meta.get("status")?,
        policy: meta.get("policy")?,
        capacity: meta.get("capacity")?,
        counts,
        occurrences,
        attendees: people,
    }))
}

#[derive(Debug, Default)]
pub struct EventQuery {
    pub from: i64,
    pub to: i64,
    pub tags: Vec<String>,
    pub collection: Option<String>,
    pub author: Option<String>,
    pub following: Option<String>,
    pub authors: Vec<String>,
    pub status: Option<String>,
    pub going: Option<String>,
    pub locations: Vec<String>,
    pub skip: usize,
    pub limit: usize,
}
pub async fn stream(q: EventQuery) -> ModelResult<Vec<Occurrence>> {
    // Select candidates from the requested indexed resource/user/collection, before
    // applying the common time and effective-RSVP filters. UNION deduplicates master hits.
    let candidates = if !q.locations.is_empty() {
        r"
        MATCH (location:Resource) USING INDEX location:Resource(id)
        WHERE location.id IN $locations
        MATCH (location)<-[:LOCATED_AT]-(located:Post)
        WITH DISTINCT located
        MATCH (o:EventOccurrence {post_uri:located.uri}) USING INDEX o:EventOccurrence(post_uri)
        WITH DISTINCT o
        "
    } else if q.going.is_some() {
        r"
        MATCH (going:User {id:$going})-[:AUTHORED]->(rsvp:Post)
        WHERE rsvp.social_kind='rsvp' AND rsvp.rsvp_status IN ['accepted','tentative']
          AND NOT coalesce(rsvp.deleted,false) AND NOT coalesce(going.deleted,false)
        CALL (rsvp) {
            MATCH (o:EventOccurrence {master:rsvp.rsvp_target}) USING INDEX o:EventOccurrence(master)
            RETURN o
            UNION
            MATCH (o:EventOccurrence {post_uri:rsvp.rsvp_target}) USING INDEX o:EventOccurrence(post_uri)
            RETURN o
        }
        WITH DISTINCT o,rsvp WHERE rsvp.recurrence_id IS NULL OR rsvp.recurrence_id=o.recurrence_id
        WITH DISTINCT o
        "
    } else if q.collection.is_some() {
        // A collection item is a series master or an override post; both reach
        // every materialized occurrence of that item.
        r"
        MATCH (c:Post {uri:$collection}) USING INDEX c:Post(uri)
        MATCH (c)-[:COLLECTED]->(item:Post)
        CALL (item) {
            MATCH (o:EventOccurrence {master:item.uri}) USING INDEX o:EventOccurrence(master)
            RETURN o
            UNION
            MATCH (o:EventOccurrence {post_uri:item.uri}) USING INDEX o:EventOccurrence(post_uri)
            RETURN o
        }
        WITH DISTINCT o
        "
    } else {
        "MATCH (o:EventOccurrence) "
    };
    let query = format!(
        "{candidates} {}",
        r"
        WHERE o.end_at > $from AND o.start_at < $to
        MATCH (p:Post {uri:o.post_uri})<-[:AUTHORED]-(u:User)
        WHERE NOT coalesce(p.deleted,false) AND NOT coalesce(u.deleted,false)
          AND ($author IS NULL OR u.id=$author)
          AND (size($authors)=0 OR u.id IN $authors)
          AND ($status IS NULL OR o.status=$status)
          AND (size($locations)=0 OR EXISTS { MATCH (p)-[:LOCATED_AT]->(location:Resource) WHERE location.id IN $locations })
          AND ($going IS NULL OR EXISTS {
              MATCH (going:User {id:$going})-[:AUTHORED]->(r:Post)
              WHERE r.social_kind='rsvp' AND r.rsvp_target IN [o.master,o.post_uri] AND NOT coalesce(r.deleted,false)
                AND NOT coalesce(going.deleted,false)
                AND (r.recurrence_id IS NULL OR r.recurrence_id=o.recurrence_id)
              WITH r,o ORDER BY CASE WHEN r.recurrence_id=o.recurrence_id OR (r.rsvp_target=o.post_uri AND o.post_uri<>o.master) THEN 1 ELSE 0 END DESC,r.social_updated_at DESC,r.id DESC LIMIT 1
              WITH r WHERE r.rsvp_status IN ['accepted','tentative'] RETURN r
          })
          AND ($following IS NULL OR EXISTS { MATCH (:User {id:$following})-[:FOLLOWS]->(u) })
          AND all(label IN $tags WHERE EXISTS {
              UNWIND [o.master,o.post_uri] AS tagged_uri
              MATCH (tagged:Post {uri:tagged_uri}) USING INDEX tagged:Post(uri)
              MATCH (tagged)<-[t:TAGGED]-(:User) WHERE t.label=label
          })
          AND ($collection IS NULL OR EXISTS {
              MATCH (c:Post {uri:$collection,kind:'collection'})-[:COLLECTED]->(item:Post)
              WHERE item.uri IN [o.master,o.post_uri] AND NOT coalesce(c.deleted,false)
          })
        RETURN properties(o) AS occurrence ORDER BY o.start_at,o.master,o.recurrence_id
        SKIP $skip LIMIT $limit
    "
    );
    let rows = fetch_all_rows_from_graph(
        Query::new("events_stream", query)
            .param("from", q.from)
            .param("to", q.to)
            .param("tags", q.tags)
            .param("collection", q.collection)
            .param("author", q.author)
            .param("authors", q.authors)
            .param("status", q.status)
            .param("going", q.going)
            .param("locations", q.locations)
            .param("following", q.following)
            .param("skip", q.skip.min(10_000) as i64)
            .param("limit", q.limit.min(200) as i64),
    )
    .await?;
    rows.iter()
        .map(|r| r.get("occurrence").map_err(ModelError::from))
        .collect()
}
