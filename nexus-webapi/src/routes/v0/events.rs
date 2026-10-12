use crate::models::{BoundedLimit, EventUsers, EventUsersQuery};
use crate::routes::AppState;
use nexus_common::models::event::EventLine;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::endpoints::{EVENTS_BY_USERS_ROUTE, EVENTS_ROUTE};

use crate::routes::Json as RequestJson;
use crate::routes::Query;
use crate::Error;
use axum::http::{HeaderName, HeaderValue};
use axum::routing::{get, post};
use axum::{response::IntoResponse, response::Response, Router};
use utoipa::OpenApi;

/// Identifies the current event log; changes when the log is recreated.
pub const EVENTS_EPOCH_HEADER: &str = "x-events-epoch";

const DEFAULT_EVENTS_LIMIT: usize = 500;
const MAX_EVENTS_LIMIT: usize = 1000;

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
    limit: Option<BoundedLimit<DEFAULT_EVENTS_LIMIT, MAX_EVENTS_LIMIT>>,
    users: Option<EventUsersQuery>,
}

#[derive(Deserialize, ToSchema)]
pub struct EventsByUsersRequest {
    /// Authors whose events to return (1–1,000 Pubky ids)
    users: EventUsers,
    /// Position in the event log to start from (default 0)
    cursor: Option<u64>,
    /// Most matching events to return (1–1000, default 500)
    limit: Option<usize>,
}

const PAGING_RULES: &str = "Filtered by author when `users` is given: only lines whose URI author \
(`pubky://<author>/...`) is listed are returned. A filtered request scans at most \
10,000 log lines, so a page can hold fewer than `limit` lines, even none, while the log \
continues: keep paging with the returned `cursor`, which is the position after the last \
line scanned. The end of the log is reached when the returned cursor equals the \
requested one. The `x-events-epoch` header identifies the log: if it changes, the log \
was recreated and a stored cursor must restart from 0.";

#[utoipa::path(
    get,
    path = EVENTS_ROUTE,
    tag = "Events",
    description = PAGING_RULES,
    params(
        ("cursor" = u64, Query, description = "Cursor"),
        ("limit" = Option<BoundedLimit<500, 1000>>, Query, description = "Number of events to return (1–1000, **default** 500)"),
        ("users" = Option<EventUsersQuery>, Query, description = "Only return events by these authors: comma-separated Pubky ids (1–100). For longer lists use `POST /v0/events/by_users`")
    ),
    responses(
        (
            status = 200,
            description = "Events list",
            body = String,
            description = "Events list as plain text with cursor",
            content_type = "text/plain",
            headers(("x-events-epoch" = String, description = "Identifies the event log; a change means the log was recreated")),
            example = "PUT pubky://<pk>/<path>\nDEL pubky://<pk>/<path>\nPUT pubky://<pk>/<path>\ncursor: 2"
        ),
        (status = 400, description = "Bad request"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error"),

    )
)]
pub async fn get_events_handler(Query(q): Query<EventsQuery>) -> Result<Response, Error> {
    let limit = q.limit.as_ref().map_or(DEFAULT_EVENTS_LIMIT, |l| l.value());
    match q.users {
        None => {
            let (events, next_cursor) = EventLine::get_from_index(q.cursor, limit).await?;
            events_response(events, next_cursor).await
        }
        Some(users) => filtered_events_response(q.cursor, limit, &users.to_author_set()).await,
    }
}

#[utoipa::path(
    post,
    path = EVENTS_BY_USERS_ROUTE,
    tag = "Events",
    description = "Events by a list of authors, for lists too long for `GET /v0/events?users=`. \
                   This is a POST request because the list of authors can hold up to 1,000 Pubky ids. \
                   Same plain-text response and paging rules as `GET /v0/events` with `users`.",
    request_body = EventsByUsersRequest,
    responses(
        (
            status = 200,
            description = "Events list as plain text with cursor",
            body = String,
            content_type = "text/plain",
            headers(("x-events-epoch" = String, description = "Identifies the event log; a change means the log was recreated")),
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
    let limit = match request.limit {
        None => DEFAULT_EVENTS_LIMIT,
        Some(0) => return Err(Error::invalid_input("limit must be at least 1")),
        Some(n) if n > MAX_EVENTS_LIMIT => {
            return Err(Error::invalid_input(format!(
                "limit exceeds maximum of {MAX_EVENTS_LIMIT}"
            )))
        }
        Some(n) => n,
    };
    filtered_events_response(request.cursor, limit, &request.users.to_author_set()).await
}

async fn filtered_events_response(
    cursor: Option<u64>,
    limit: usize,
    authors: &std::collections::HashSet<String>,
) -> Result<Response, Error> {
    let page = EventLine::get_filtered_from_index(cursor, limit, authors).await?;
    events_response(page.lines, page.next_cursor).await
}

/// The plain-text event list, with the log epoch header.
async fn events_response(events: Vec<String>, cursor: u64) -> Result<Response, Error> {
    let epoch = EventLine::epoch().await?;
    let body = EventsList { events, cursor }.to_string();
    let mut response = body.into_response();
    if let Ok(value) = HeaderValue::from_str(&epoch) {
        response
            .headers_mut()
            .insert(HeaderName::from_static(EVENTS_EPOCH_HEADER), value);
    }
    Ok(response)
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(EVENTS_ROUTE, get(get_events_handler))
        .route(EVENTS_BY_USERS_ROUTE, post(post_events_by_users_handler))
}

#[derive(OpenApi)]
#[openapi(
    paths(get_events_handler, post_events_by_users_handler),
    components(schemas(EventsList, EventsByUsersRequest, EventUsers, EventUsersQuery))
)]
pub struct EventsApiDoc;
