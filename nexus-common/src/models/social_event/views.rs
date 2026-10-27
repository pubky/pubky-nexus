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

async fn attendance(
    uri: &str,
    recurrence: Option<&str>,
) -> ModelResult<(Vec<Attendee>, EventCounts)> {
    let meta = fetch_row_from_graph(
        Query::new(
            "attendance_policy",
            "MATCH (p:Post {uri:$uri}) WHERE p.event_content IS NOT NULL
        OPTIONAL MATCH (o:EventOccurrence {master:$uri,recurrence_id:$recurrence})
        OPTIONAL MATCH (override:Post {uri:o.post_uri})
        RETURN coalesce(override,p).policy AS policy,coalesce(override,p).capacity AS capacity,
          coalesce(override,p).waitlist AS waitlist,coalesce(override,p).uri AS effective",
        )
        .param("uri", uri)
        .param("recurrence", recurrence),
    )
    .await?;
    let Some(meta) = meta else {
        return Ok((vec![], EventCounts::default()));
    };
    let policy: String = meta.get("policy")?;
    let capacity: Option<i64> = meta.get("capacity")?;
    let waitlist: bool = meta.get("waitlist")?;
    let rows=fetch_all_rows_from_graph(Query::new("event_attendance", r"
        MATCH (u:User)-[:AUTHORED]->(r:Post)
        WHERE r.social_kind='rsvp' AND r.rsvp_target IN [$uri,$effective] AND NOT coalesce(r.deleted,false)
          AND (r.recurrence_id IS NULL OR r.recurrence_id=$recurrence)
        WITH u,r ORDER BY CASE WHEN r.recurrence_id=$recurrence OR (r.rsvp_target=$effective AND $effective<>$uri) THEN 1 ELSE 0 END DESC,r.social_updated_at DESC,r.id DESC
        WITH u,collect(r)[0] AS r
        RETURN u.id AS author,r.uri AS post_uri,r.rsvp_status AS status,r.recurrence_id AS recurrence_id,
               r.guests AS guests,r.indexed_at AS indexed_at,false AS waitlisted
        ORDER BY r.indexed_at,u.id
    ").param("uri",uri).param("effective",meta.get::<String>("effective")?).param("recurrence",recurrence)).await?;
    let mut people = Vec::new();
    let mut counts = EventCounts::default();
    let mut seats = 0;
    for row in rows {
        let mut person: Attendee = Attendee {
            author: row.get("author")?,
            post_uri: row.get("post_uri")?,
            status: row.get("status")?,
            recurrence_id: row.get("recurrence_id")?,
            guests: row.get("guests")?,
            indexed_at: row.get("indexed_at")?,
            waitlisted: false,
        };
        match person.status.as_str() {
            "requested" => counts.requested += 1,
            "declined" => counts.declined += 1,
            "tentative" => counts.tentative += 1,
            "accepted" => {
                counts.accepted += 1;
                let size = 1 + if policy == "ticket" { 0 } else { person.guests };
                seats += size;
                counts.going += size;
                person.waitlisted = waitlist && capacity.is_some_and(|cap| seats > cap);
                if person.waitlisted {
                    counts.waitlisted += size;
                }
            }
            _ => {}
        }
        people.push(person);
    }
    Ok((people, counts))
}

pub async fn attendees(
    uri: &str,
    recurrence: Option<&str>,
    status: Option<&str>,
    skip: usize,
    limit: usize,
) -> ModelResult<Vec<Attendee>> {
    let (people, _) = attendance(uri, recurrence).await?;
    Ok(people
        .into_iter()
        .filter(|a| status.is_none_or(|s| a.status == s))
        .skip(skip.min(10_000))
        .take(limit.min(200))
        .collect())
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
        MATCH (:User {id:$author})-[:AUTHORED]->(p:Post {id:$id})
        WHERE p.event_content IS NOT NULL AND NOT coalesce(p.deleted,false)
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
    let (people, counts) = attendance(&uri, recurrence).await?;
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
        attendees: people
            .into_iter()
            .take(options.attendees.min(200))
            .collect(),
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
    // Select candidates from the requested indexed resource/user, before applying
    // the common time and effective-RSVP filters. UNION deduplicates master hits.
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
        MATCH (:User {id:$going})-[:AUTHORED]->(rsvp:Post)
        WHERE rsvp.social_kind='rsvp' AND rsvp.rsvp_status IN ['accepted','tentative']
          AND NOT coalesce(rsvp.deleted,false)
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
              MATCH (:User {id:$going})-[:AUTHORED]->(r:Post)
              WHERE r.social_kind='rsvp' AND r.rsvp_target IN [o.master,o.post_uri] AND NOT coalesce(r.deleted,false)
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
