use crate::models::BoundedLimit;
use crate::routes::AppState;
use nexus_common::models::event::EventLine;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::endpoints::EVENTS_ROUTE;

use crate::routes::Query;
use crate::Error;
use axum::routing::get;
use axum::{response::Response, Router};
use utoipa::OpenApi;

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
    limit: Option<BoundedLimit<500, 1000>>,
}

#[utoipa::path(
    get,
    path = EVENTS_ROUTE,
    tag = "Events",
    params(
        ("cursor" = u64, Query, description = "Cursor"),
        ("limit" = Option<BoundedLimit<500, 1000>>, Query, description = "Number of events to return (1–1000, **default** 500)")
    ),
    responses(
        (
            status = 200,
            description = "Events list",
            body = String,
            description = "Events list as plain text with cursor. Each line ends with the public key of the homeserver the event was read from, after the last space: the path may contain spaces. Some lines stored before this key was added have none, and end with the path.",
            content_type = "text/plain",
            example = "PUT pubky://<pk>/<path> <hs_pk>\nDEL pubky://<pk>/<path> <hs_pk>\nPUT pubky://<pk>/<path> <hs_pk>\ncursor: 2"
        ),
        (status = 400, description = "Bad request"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error"),

    )
)]
pub async fn get_events_handler(Query(q): Query<EventsQuery>) -> Result<Response, Error> {
    let limit = q.limit.as_ref().map_or(500, |l| l.value());
    let cursor = q.cursor;
    let (events, next_cursor) = EventLine::get_from_index(cursor, limit).await?;
    let event_list = EventsList {
        events,
        cursor: next_cursor,
    };

    // Convert to a plain text response
    let response: Response = axum::response::IntoResponse::into_response(event_list.to_string());
    Ok(response)
}

pub fn routes() -> Router<AppState> {
    Router::new().route(EVENTS_ROUTE, get(get_events_handler))
}

#[derive(OpenApi)]
#[openapi(paths(get_events_handler), components(schemas(EventsList)))]
pub struct EventsApiDoc;
