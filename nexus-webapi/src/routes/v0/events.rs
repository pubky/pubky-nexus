use crate::routes::AppState;
use nexus_common::models::event::Event;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::endpoints::EVENTS_ROUTE;
use crockford;

use crate::Error;
use axum::extract::Query;
use axum::routing::get;
use axum::{response::Response, Router};
use utoipa::OpenApi;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EventsList {
    cursor: u64,
    events: Vec<String>,
}

impl std::fmt::Display for EventsList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for line in &self.events {
            writeln!(f, "{}", line)?;
        }
        write!(f, "cursor: {:0>13}", crockford::encode(self.cursor))
    }
}

#[derive(Deserialize)]
pub struct EventsQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

fn decode_crockford32(s: &str) -> Result<i64, String> {
    crockford::decode(s)
        .map(|v| v as i64)
        .map_err(|_e| format!("Invalid cursor {}", s))
}

#[utoipa::path(
    get,
    path = EVENTS_ROUTE,
    tag = "Events",
    params(
        ("cursor" = String, Query, description = "Cursor"),
        ("limit" = usize, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Events list", body = EventsList),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
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

fn parse_query(q: &EventsQuery) -> Result<(usize, Option<f64>), Error> {
    let limit = q.limit.unwrap_or(500);

    let cursor = match q.cursor.as_deref() {
        None => None,
        Some(c) => match decode_crockford32(c) {
            Ok(score) => Some(score as f64),
            Err(e) => return Err(Error::InvalidInput { message: e }),
        },
    };

    Ok((limit, cursor))
}

pub fn routes() -> Router<AppState> {
    Router::new().route(EVENTS_ROUTE, get(get_events_handler))
}

#[derive(OpenApi)]
#[openapi(paths(get_events_handler), components(schemas(EventsList)))]
pub struct EventsApiDoc;
