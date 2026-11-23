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
    /// Whether each line keeps the key of the homeserver the event was read from
    homeserver: Option<bool>,
}

#[utoipa::path(
    get,
    path = EVENTS_ROUTE,
    tag = "Events",
    params(
        ("cursor" = u64, Query, description = "Cursor"),
        ("limit" = Option<BoundedLimit<500, 1000>>, Query, description = "Number of events to return (1–1000, **default** 500)"),
        ("homeserver" = Option<bool>, Query, description = "When `true`, each line is returned as stored: the public key of the homeserver Nexus read the event from, then the line the homeserver sent, `<hs_pk> PUT pubky://<pk>/<path>`. A reader takes the first field as the key when it is not `PUT` or `DEL`; a path may contain spaces and key-shaped words, but they always come after. Lines stored before Nexus kept the key, whose user's homeserver the graph did not know, have none and are returned without one, `PUT pubky://<pk>/<path>`. The key says where Nexus read the event, not where the user is hosted, and on a line stored before Nexus kept the key it is inferred: the homeserver the user was bound to when the key was added. Treat it as a hint. `false` (default) strips the key from every line, which is the output this endpoint always had.")
    ),
    responses(
        (
            status = 200,
            description = "Events list",
            body = String,
            description = "Events list as plain text: one `METHOD pubky://<pk>/<path>` line per event, with `<hs_pk> ` in front of the lines that have a key when `homeserver=true`, then the `cursor:` line. The cursor, the limit and the offsets are the same in both modes.",
            content_type = "text/plain",
            example = "PUT pubky://<pk>/<path>\nDEL pubky://<pk>/<path>\nPUT pubky://<pk>/<path>\ncursor: 2"
        ),
        (status = 400, description = "Bad request"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error"),

    )
)]
pub async fn get_events_handler(Query(q): Query<EventsQuery>) -> Result<Response, Error> {
    let limit = q.limit.as_ref().map_or(500, |l| l.value());
    let cursor = q.cursor;
    let (mut events, next_cursor) = EventLine::get_from_index(cursor, limit).await?;
    if !q.homeserver.unwrap_or(false) {
        // The line as the homeserver sent it: what this endpoint served before the key was
        // stored. A line without a key is unchanged.
        events = events
            .iter()
            .map(|line| EventLine::split(line).0.to_string())
            .collect();
    }
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
