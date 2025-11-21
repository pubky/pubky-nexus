use crate::routes::AppState;
use nexus_common::models::event::Event;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::endpoints::EVENTS_ROUTE;

use crate::Error;
use axum::extract::Query;
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
    limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = EVENTS_ROUTE,
    tag = "Events",
    params(
        ("cursor" = u64, Query, description = "Cursor"),
        ("limit" = usize, Query, description = "Limit the number of results, (default 500, maximum 1000)")
    ),
    responses(
        (
            status = 200,
            description = "Events list",
            body = String,
            description = "Events list as plain text with cursor",
            content_type = "text/plain",
            example = "PUT pubky://<pk>/<path>\nDEL pubky://<pk>/<path>\nPUT pubky://<pk>/<path>\ncursor: 2"
        ),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error"),

    )
)]
pub async fn get_events_handler(Query(q): Query<EventsQuery>) -> Result<Response, Error> {
    let (limit, cursor) = parse_query(&q)?;
    let items = Event::get_events_from_redis(cursor, limit)
        .await
        .map_err(|source| Error::InternalServerError { source })?;
    let event_list = assemble_page(items);

    // Convert to a plain text response
    let response: Response = axum::response::IntoResponse::into_response(event_list.to_string());
    Ok(response)
}

fn assemble_page(items: Vec<(String, f64)>) -> EventsList {
    let mut events = Vec::with_capacity(items.len());
    let mut cursor: u64 = 0;
    if !items.is_empty() {
        // if line is the last one, set the cursor to its score, for the rest of the items collect them in events
        let len = items.len();
        for (i, (line, score)) in items.into_iter().enumerate() {
            if i == len - 1 {
                cursor = score as u64;
            } else {
                events.push(line);
            }
        }
    }

    EventsList { events, cursor }
}

fn parse_query(q: &EventsQuery) -> Result<(usize, f64), Error> {
    let limit = q.limit.unwrap_or(500).min(1000);
    let cursor = q.cursor.unwrap_or(0) as f64;

    Ok((limit, cursor))
}

pub fn routes() -> Router<AppState> {
    Router::new().route(EVENTS_ROUTE, get(get_events_handler))
}

#[derive(OpenApi)]
#[openapi(paths(get_events_handler), components(schemas(EventsList)))]
pub struct EventsApiDoc;
