use crate::models::{BoundedLimit, BoundedPagination, PostId, PubkyId, ResourceId, Tags};
use crate::routes::{AppState, Path, Query};
use crate::{Error, Result};
use axum::{routing::get, Json, Router};
use nexus_common::models::social_event::{self, Attendee, EventQuery, EventView, Occurrence};
use serde::Deserialize;
use utoipa::{OpenApi, ToSchema};

#[derive(Default, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventSource {
    #[default]
    All,
    Following,
}
#[derive(Default, Deserialize)]
pub struct StreamQuery {
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub tags: Option<Tags>,
    pub collection: Option<String>,
    pub author: Option<PubkyId>,
    pub authors: Option<String>,
    pub status: Option<String>,
    pub going: Option<PubkyId>,
    pub locations: Option<String>,
    #[serde(default)]
    pub source: EventSource,
    pub user_id: Option<PubkyId>,
    pub sort: Option<String>,
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 200>,
}
#[derive(Default, Deserialize)]
pub struct AttendanceQuery {
    pub recurrence_id: Option<String>,
    pub status: Option<String>,
    pub viewer_id: Option<PubkyId>,
    pub limit_tags: Option<BoundedLimit<10, 100>>,
    pub limit_taggers: Option<BoundedLimit<10, 100>>,
    pub limit_attendees: Option<BoundedLimit<20, 200>>,
    pub limit_occurrences: Option<BoundedLimit<50, 50>>,
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 200>,
}
fn check_recurrence(value: Option<&str>) -> Result<()> {
    if let Some(value) = value {
        social_event::calendar::local(value).map_err(|_| {
            Error::invalid_input("recurrence_id must be a local YYYY-MM-DDTHH:MM:SS")
        })?;
    }
    Ok(())
}

#[utoipa::path(get,path="/v0/stream/events",tag="Events",
    description="Indexed social event occurrences in start order. Times are UTC milliseconds. Returns occurrences overlapping [from, to): end_at > from AND start_at < to. Defaults to now through twelve months ahead, including events already in progress. Recurrences are materialized on writes and rolled daily; requests never expand rules. One row per master/recurrence_id, with post_uri naming its current override. locations matches any indexed LOCATED_AT Resource on the effective event or override. going selects accepted or tentative effective RSVPs per occurrence; an explicit occurrence answer overrides the series answer.",
    params(
      ("from"=Option<i64>,Query),("to"=Option<i64>,Query),
      ("tags"=Option<Tags>,Query,description="All supplied tags must match the master or override"),
      ("collection"=Option<String>,Query,description="Versionless public collection URI"),
      ("author"=Option<PubkyId>,Query),("authors"=Option<String>,Query,description="Comma-separated author public keys, 1–100; intersects author when both are supplied"),
      ("status"=Option<String>,Query,description="confirmed, tentative or cancelled"),
      ("going"=Option<PubkyId>,Query,description="User with an accepted or tentative effective RSVP for each occurrence"),
      ("locations"=Option<String>,Query,description="1–100 comma-separated Resource IDs or URIs. CSV-quote URIs containing commas (for example geo: coordinates), then URL-encode the parameter. Matches any location of the effective event."),
      ("source"=Option<EventSource>,Query),
      ("user_id"=Option<PubkyId>,Query,description="Required for source=following"),
      ("sort"=Option<String>,Query,description="start (ascending)"),
      ("skip"=Option<usize>,Query),("limit"=Option<usize>,Query,description="1–200, default 20")
    ),responses((status=200,body=Vec<Occurrence>),(status=400,description="Invalid query"),(status=500,description="Internal error")))]
pub async fn events(Query(q): Query<StreamQuery>) -> Result<Json<Vec<Occurrence>>> {
    let now = chrono::Utc::now();
    let from = q.from.unwrap_or(now.timestamp_millis());
    let to =
        q.to.unwrap_or_else(|| social_event::calendar::window_end(now));
    if from < 0 || to <= from || to.saturating_sub(from) > 366 * 86_400_000 {
        return Err(Error::invalid_input(
            "from/to must be nonnegative UTC milliseconds with from < to and a span of at most 366 days",
        ));
    }
    if q.sort.as_deref().is_some_and(|s| s != "start") {
        return Err(Error::invalid_input("Events sort must be start"));
    }
    if q.status
        .as_deref()
        .is_some_and(|s| !matches!(s, "confirmed" | "tentative" | "cancelled"))
    {
        return Err(Error::invalid_input("Invalid event status"));
    }
    let locations = q
        .locations
        .as_deref()
        .map(location_ids)
        .transpose()?
        .unwrap_or_default();
    let authors = q
        .authors
        .as_deref()
        .map(|raw| {
            csv_items(raw)?
                .into_iter()
                .map(|id| {
                    PubkyId::try_from(&id)
                        .map(|v| v.to_string())
                        .map_err(Error::invalid_input)
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();
    let following = match q.source {
        EventSource::All => None,
        EventSource::Following => Some(
            q.user_id
                .ok_or_else(|| Error::invalid_input("source=following requires user_id"))?
                .to_string(),
        ),
    };
    if let Some(uri) = &q.collection {
        let parsed = pubky_social_specs::ParsedUri::try_from(uri.as_str()).map_err(|_| {
            Error::invalid_input("collection must be a public versionless post URI")
        })?;
        if parsed.visibility != pubky_social_specs::Visibility::Public
            || !matches!(
                parsed.resource,
                pubky_social_specs::Resource::Post { version: None, .. }
            )
        {
            return Err(Error::invalid_input(
                "collection must be a public versionless post URI",
            ));
        }
    }
    Ok(Json(
        social_event::stream(EventQuery {
            from,
            to,
            tags: q.tags.map(|t| t.to_string_vec()).unwrap_or_default(),
            collection: q.collection,
            author: q.author.map(|u| u.to_string()),
            following,
            authors,
            status: q.status,
            going: q.going.map(|v| v.to_string()),
            locations,
            skip: q.pagination.skip_value(),
            limit: q.pagination.limit_value(),
        })
        .await?,
    ))
}

#[utoipa::path(get,path="/v0/event/{author}/{id}",tag="Events",
    description="A native social event with attendance counts and up to 50 next indexed occurrences. recurrence_id selects an occurrence's effective RSVP counts; its explicit RSVP overrides the author's series RSVP. Going includes accepted attendees and guests, except ticket policy ignores guests. Requested is never counted as going.",
    params(("author"=PubkyId,Path),("id"=PostId,Path),("recurrence_id"=Option<String>,Query),
      ("viewer_id"=Option<PubkyId>,Query),("limit_tags"=Option<BoundedLimit<10,100>>,Query),("limit_taggers"=Option<BoundedLimit<10,100>>,Query),
      ("limit_attendees"=Option<BoundedLimit<20,200>>,Query),("limit_occurrences"=Option<BoundedLimit<50,50>>,Query)),
    responses((status=200,body=EventView),(status=400,description="Invalid query"),(status=404,description="Event not found"),(status=500,description="Internal error")))]
pub async fn event(
    Path((author, id)): Path<(PubkyId, PostId)>,
    Query(q): Query<AttendanceQuery>,
) -> Result<Json<EventView>> {
    check_recurrence(q.recurrence_id.as_deref())?;
    let view = social_event::event_view_with_options(
        &author,
        &id,
        q.recurrence_id.as_deref(),
        social_event::EventViewOptions {
            viewer: q.viewer_id.map(|v| v.to_string()),
            tags: q.limit_tags.unwrap_or_default().value(),
            taggers: q.limit_taggers.unwrap_or_default().value(),
            attendees: q.limit_attendees.unwrap_or_default().value(),
            occurrences: q.limit_occurrences.unwrap_or_default().value(),
        },
    )
    .await?
    .ok_or_else(|| Error::post_not_found(author, id))?;
    Ok(Json(view))
}

#[utoipa::path(get,path="/v0/event/{author}/{id}/attendees",tag="Events",
    description="One effective RSVP per author, ordered by original indexed_at then author (FIFO waitlist order). Newest indexed content wins across duplicate RSVP posts; retries do not move the attendee. An occurrence-specific answer supersedes the series answer. Guests count as seats except with ticket policy. waitlisted is informational; Nexus grants no admission rights.",
    params(("author"=PubkyId,Path),("id"=PostId,Path),("recurrence_id"=Option<String>,Query),("status"=Option<String>,Query,description="requested, accepted, declined or tentative"),("skip"=Option<usize>,Query),("limit"=Option<usize>,Query)),
    responses((status=200,body=Vec<Attendee>),(status=400,description="Invalid query"),(status=404,description="Event not found"),(status=500,description="Internal error")))]
pub async fn attendees(
    Path((author, id)): Path<(PubkyId, PostId)>,
    Query(q): Query<AttendanceQuery>,
) -> Result<Json<Vec<Attendee>>> {
    check_recurrence(q.recurrence_id.as_deref())?;
    if q.status
        .as_deref()
        .is_some_and(|s| !matches!(s, "requested" | "accepted" | "declined" | "tentative"))
    {
        return Err(Error::invalid_input("Invalid RSVP status"));
    }
    let uri = pubky_social_specs::post_uri_builder(author.to_string(), id.to_string());
    let key = nexus_common::models::ResourceKey {
        owner_id: author.clone(),
        resource_id: id.to_string(),
    };
    if social_event::validate_parent(&key).await? != Some(true) {
        return Err(Error::post_not_found(author, id));
    }
    Ok(Json(
        social_event::attendees(
            &uri,
            q.recurrence_id.as_deref(),
            q.status.as_deref(),
            q.pagination.skip_value(),
            q.pagination.limit_value(),
        )
        .await?,
    ))
}
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v0/stream/availability", get(availability))
        .route("/v0/availability/{author}", get(author_availability))
        .route("/v0/stream/events", get(events))
        .route("/v0/event/{author}/{id}", get(event))
        .route("/v0/event/{author}/{id}/attendees", get(attendees))
}
#[derive(OpenApi)]
#[openapi(
    paths(
        events,
        event,
        attendees,
        availability,
        author_availability
    ),
    components(schemas(
        EventSource,
        Occurrence,
        EventView,
        Attendee,
        social_event::AvailabilityView,
        BoundedLimit<10,100>, BoundedLimit<20,200>, BoundedLimit<50,50>
    ))
)]
pub struct SocialEventsApiDoc;

/// Bounded single-record CSV. Quoting makes commas inside universal URIs unambiguous.
fn csv_items(raw: &str) -> Result<Vec<String>> {
    if raw.len() > 205_100 {
        return Err(Error::invalid_input("Filter is too long"));
    }
    let mut values = Vec::new();
    let mut chars = raw.trim().chars().peekable();
    loop {
        let mut value = String::new();
        while chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            chars.next();
        }
        if chars.peek() == Some(&'"') {
            chars.next();
            loop {
                match chars.next() {
                    Some('"') if chars.peek() == Some(&'"') => {
                        chars.next();
                        value.push('"');
                    }
                    Some('"') => break,
                    Some(c) => value.push(c),
                    None => return Err(Error::invalid_input("Unclosed CSV quote")),
                }
            }
            while chars.peek().is_some_and(|c| c.is_ascii_whitespace()) {
                chars.next();
            }
            if chars.peek().is_some_and(|c| *c != ',') {
                return Err(Error::invalid_input("Unexpected text after CSV quote"));
            }
        } else {
            while chars.peek().is_some_and(|c| *c != ',') {
                value.push(chars.next().unwrap());
            }
            if value.contains('"') {
                return Err(Error::invalid_input(
                    "CSV quotes must surround an entire value",
                ));
            }
        }
        let value = value.trim();
        if value.is_empty() || value.len() > 2048 {
            return Err(Error::invalid_input(
                "Filter entries must contain 1–2048 bytes",
            ));
        }
        values.push(value.to_owned());
        if values.len() > 100 {
            return Err(Error::invalid_input(
                "At most 100 filter entries are allowed",
            ));
        }
        match chars.next() {
            Some(',') => {}
            None => break,
            _ => unreachable!(),
        }
    }
    Ok(values)
}
fn location_ids(raw: &str) -> Result<Vec<String>> {
    use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
    csv_items(raw)?
        .into_iter()
        .map(|value| {
            if ResourceId::validate_id(&value).is_ok() {
                Ok(value)
            } else {
                normalize_uri(&value)
                    .map(|(uri, _)| resource_id(&uri))
                    .map_err(Error::invalid_input)
            }
        })
        .collect()
}
#[derive(Default, Deserialize)]
pub struct AvailabilityQuery {
    pub author: Option<PubkyId>,
    pub r#for: Option<String>,
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 200>,
}
#[utoipa::path(get,path="/v0/stream/availability",tag="Events",
    description="Newest live social availability post per author and normalized for URI. Require author, for, or both. An absent for in a post means pubky://author. Deletes and kind/target changes reveal the next newest surviving schedule. No slots or booking rights are inferred.",
    params(("author"=Option<PubkyId>,Query),("for"=Option<String>,Query),("skip"=Option<usize>,Query),("limit"=Option<BoundedLimit<20,200>>,Query)),
    responses((status=200,body=Vec<social_event::AvailabilityView>),(status=400,description="Invalid query"),(status=500,description="Internal error")))]
pub async fn availability(
    Query(q): Query<AvailabilityQuery>,
) -> Result<Json<Vec<social_event::AvailabilityView>>> {
    if q.author.is_none() && q.r#for.is_none() {
        return Err(Error::invalid_input("author or for is required"));
    }
    if q.r#for.as_ref().is_some_and(|v| v.len() > 2048) {
        return Err(Error::invalid_input("for exceeds 2048 bytes"));
    }
    Ok(Json(
        social_event::availability_stream(
            q.author.as_deref(),
            q.r#for.as_deref(),
            q.pagination.skip_value(),
            q.pagination.limit_value(),
        )
        .await?,
    ))
}
#[utoipa::path(get,path="/v0/availability/{author}",tag="Events",
    description="Current availability schedules by author, optionally restricted to one normalized for URI.",
    params(("author"=PubkyId,Path),("for"=Option<String>,Query),("skip"=Option<usize>,Query),("limit"=Option<BoundedLimit<20,200>>,Query)),
    responses((status=200,body=Vec<social_event::AvailabilityView>),(status=400,description="Invalid query"),(status=500,description="Internal error")))]
pub async fn author_availability(
    Path(author): Path<PubkyId>,
    Query(mut q): Query<AvailabilityQuery>,
) -> Result<Json<Vec<social_event::AvailabilityView>>> {
    q.author = Some(author);
    availability(Query(q)).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locations_accept_ids_and_quoted_universal_uris_with_bounds() {
        use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
        let id = "0123456789abcdef0123456789abcdef";
        let ids =
            location_ids(&format!("{id},https://EXAMPLE.com/place,\"geo:47.2,8.3\"")).unwrap();
        assert_eq!(ids[0], id);
        assert_eq!(ids[1], resource_id("https://example.com/place"));
        assert_eq!(
            ids[2],
            resource_id(&normalize_uri("geo:47.2,8.3").unwrap().0)
        );
        assert!(location_ids(&vec![id; 100].join(",")).is_ok());
        for invalid in [
            "".into(),
            "https://example.com/x,".into(),
            "relative".into(),
            "\"geo:1,2".into(),
            "\"geo:1,2\"x".into(),
            vec![id; 101].join(","),
            format!("https://example.com/{}", "x".repeat(2048)),
        ] {
            assert!(location_ids(&invalid).is_err(), "{invalid}");
        }
    }
}
