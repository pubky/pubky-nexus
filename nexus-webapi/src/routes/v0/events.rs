use crate::models::{BoundedLimit, EventUsers};
use crate::routes::AppState;
use nexus_common::models::event::{EventLine, EventLogState};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::endpoints::{EVENTS_BY_USERS_ROUTE, EVENTS_ROUTE};

use crate::routes::Json as RequestJson;
use crate::routes::Query;
use crate::Error;
use axum::http::HeaderValue;
use axum::routing::{get, post};
use axum::{response::IntoResponse, response::Response, Router};
use const_format::formatcp;
use nexus_common::models::event::EVENTS_FILTER_SCAN_BUDGET;
use tracing::warn;
use utoipa::OpenApi;

/// Identifies the current event log; changes when the log is recreated.
pub const EVENTS_EPOCH_HEADER: &str = "x-events-epoch";
/// Number of lines in the event log when the request started.
pub const EVENTS_LENGTH_HEADER: &str = "x-events-length";

const DEFAULT_EVENTS_LIMIT: usize = 500;
const MAX_EVENTS_LIMIT: usize = 1000;
type EventsLimit = BoundedLimit<DEFAULT_EVENTS_LIMIT, MAX_EVENTS_LIMIT>;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, utoipa::ToResponse)]
#[schema(as = String)]
pub struct EventsList {
    cursor: u64,
    events: Vec<String>,
}

impl std::fmt::Display for EventsList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for line in &self.events {
            writeln!(f, "{}", line)?;
        }
        write!(f, "cursor: {}", self.cursor)
    }
}

#[derive(Deserialize)]
pub struct EventsQuery {
    cursor: Option<u64>,
    limit: Option<EventsLimit>,
    /// Not supported here: filtering by author is `POST /v0/events/by_users`. Kept as a
    /// field only to reject it, rather than silently returning the whole log.
    users: Option<String>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EventsByUsersRequest {
    /// Authors whose events to return (1–1,000 Pubky ids)
    #[schema(value_type = Vec<String>, min_items = 1, max_items = 1000)]
    users: EventUsers,
    /// Position in the event log to start from (default 0)
    cursor: Option<u64>,
    /// Most matching events to return (1–1000, default 500)
    #[schema(minimum = 1, maximum = 1000)]
    limit: Option<usize>,
}

/// How a follower detects that its stored cursor no longer fits the log.
const RESTART_RULES: &str = "Restart from cursor 0 when `x-events-epoch` changes from one \
value to another (keep your stored epoch while it is absent) or when the cursor you sent is \
past `x-events-length`. Never compare the returned cursor with `x-events-length`.";

const GET_DESCRIPTION: &str = formatcp!(
    "Every indexed event, in order. To follow only some authors, use \
`POST /v0/events/by_users`. {RESTART_RULES}"
);

const POST_DESCRIPTION: &str = formatcp!(
    "Events by up to 1,000 authors, in the same plain-text format as `GET /v0/events`. A \
request scans at most {EVENTS_FILTER_SCAN_BUDGET} lines, so a page can hold fewer than \
`limit` lines, even none, before the log ends: keep paging with the returned `cursor` (the \
position after the last line scanned) until it equals the one you sent. {RESTART_RULES}"
);

#[utoipa::path(
    get,
    path = EVENTS_ROUTE,
    tag = "Events",
    description = GET_DESCRIPTION,
    params(
        ("cursor" = u64, Query, description = "Cursor"),
        ("limit" = Option<BoundedLimit<500, 1000>>, Query, description = "Number of events to return (1–1000, **default** 500)")
    ),
    responses(
        (
            status = 200,
            description = "Events list",
            body = String,
            description = "Events list as plain text with cursor",
            content_type = "text/plain",
            headers(
                ("x-events-epoch" = String, description = "Identifies the event log; a change means the log was recreated. Absent until the first event is stored"),
                ("x-events-length" = u64, description = "Lines in the event log when the request started; a sent cursor past it means the log was truncated or restored")
            ),
            example = "PUT pubky://<pk>/<path>\nDEL pubky://<pk>/<path>\nPUT pubky://<pk>/<path>\ncursor: 2"
        ),
        (status = 400, description = "Bad request"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error"),

    )
)]
pub async fn get_events_handler(Query(q): Query<EventsQuery>) -> Result<Response, Error> {
    if q.users.is_some() {
        return Err(Error::invalid_input(
            "filtering by author is not supported here: use POST /v0/events/by_users",
        ));
    }
    let limit = q.limit.as_ref().map_or(DEFAULT_EVENTS_LIMIT, |l| l.value());
    let log = read_log_state().await;
    let (events, next_cursor) = EventLine::get_from_index(q.cursor, limit).await?;
    Ok(events_response(events, next_cursor, log))
}

#[utoipa::path(
    post,
    path = EVENTS_BY_USERS_ROUTE,
    tag = "Events",
    description = POST_DESCRIPTION,
    request_body = EventsByUsersRequest,
    responses(
        (
            status = 200,
            description = "Events list as plain text with cursor",
            body = String,
            content_type = "text/plain",
            headers(
                ("x-events-epoch" = String, description = "Identifies the event log; a change means the log was recreated. Absent until the first event is stored"),
                ("x-events-length" = u64, description = "Lines in the event log when the request started; a sent cursor past it means the log was truncated or restored")
            ),
            example = "PUT pubky://<pk>/<path>\ncursor: 2"
        ),
        (status = 400, description = "Bad request"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn post_events_by_users_handler(
    RequestJson(request): RequestJson<EventsByUsersRequest>,
) -> Result<Response, Error> {
    let limit = request.limit.unwrap_or(DEFAULT_EVENTS_LIMIT);
    EventsLimit::validate(limit)?;
    let log = read_log_state().await;
    let page = EventLine::get_filtered_from_index(request.cursor, limit, &request.users.0).await?;
    Ok(events_response(page.lines, page.next_cursor, log))
}

/// The log's epoch and length, read before the events so that a log recreated
/// mid-request is reported with the old identity and the follower notices on its
/// next page. Never fails the request: if Redis can't answer, the headers are left out.
async fn read_log_state() -> Option<EventLogState> {
    EventLine::log_state()
        .await
        .inspect_err(|e| warn!("Could not read the event log epoch and length: {e}"))
        .ok()
}

/// The plain-text event list, with the log's epoch and length headers.
fn events_response(events: Vec<String>, cursor: u64, log: Option<EventLogState>) -> Response {
    let body = EventsList { events, cursor }.to_string();
    let mut response = body.into_response();
    if let Some(log) = log {
        let headers = response.headers_mut();
        if let Some(epoch) = log.epoch.and_then(|e| HeaderValue::from_str(&e).ok()) {
            headers.insert(EVENTS_EPOCH_HEADER, epoch);
        }
        headers.insert(EVENTS_LENGTH_HEADER, log.len.into());
    }
    response
}

pub fn routes() -> Router<AppState> {
    Router::new().route(EVENTS_ROUTE, get(get_events_handler))
}

/// Filtering scans up to the whole budget per request, so it is only offered as this
/// `POST` route, in the expensive rate-limit bucket.
pub fn expensive_routes() -> Router<AppState> {
    Router::new().route(EVENTS_BY_USERS_ROUTE, post(post_events_by_users_handler))
}

#[derive(OpenApi)]
#[openapi(
    paths(get_events_handler, post_events_by_users_handler),
    components(schemas(EventsList, EventsByUsersRequest, EventUsers))
)]
pub struct EventsApiDoc;
