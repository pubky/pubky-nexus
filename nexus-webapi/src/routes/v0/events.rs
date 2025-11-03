use crate::routes::AppState;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::endpoints::EVENTS_ROUTE;

use axum::extract::Query;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EventsList {
    cursor: String,
    events: Vec<String>,
}

#[derive(Deserialize)]
struct EventsQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

// Minimal Crockford32 <-> millis helpers; if you already use pubky-timestamp, prefer that.
fn encode_crockford32(ts_ms: i64) -> String {
    // use a proper Crockford32 encoder; placeholder:
    pubky_timestamp::to_z32(ts_ms as u64) // example, adjust to actual crate API
}
fn decode_crockford32(s: &str) -> Option<i64> {
    // use a proper Crockford32 decoder; placeholder:
    pubky_timestamp::from_z32(s).ok().map(|v| v as i64)
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
pub async fn get_events_handler(
    Query(_cursor): Query<String>,
    Query(_limit): Query<usize>,
) -> impl IntoResponse {
    let start_score: Option<f64> = match q.cursor.as_deref() {
        Some(c) => decode_crockford32(c).map(|v| v as f64),
        None => None, // read from the beginning
    };

    // 2) Query Redis ZSET
    let limit = q.limit.or(Some(50)); // pick a sensible default
    let result = Event::try_from_index_sorted_set(
        &["Events"],
        start_score, // start (exclusive/inclusive is handled by the ZRANGEBYSCORE impl)
        None,        // end
        None,        // skip
        limit,       // limit
        SortOrder::Asc,
        None, // prefix -> "Sorted"
    )
    .await;

    let items = match result {
        Ok(Some(v)) => v, // Vec<(String, f64)>
        Ok(None) => Vec::new(),
        Err(_e) => Vec::new(), // surface error as empty / or return 500 as you prefer
    };

    // 3) Convert to response: events + next cursor
    let mut events = Vec::with_capacity(items.len());
    let mut next_cursor = q.cursor.unwrap_or_default();
    if !items.is_empty() {
        for (line, _score) in &items {
            events.push(line.clone()); // "PUT ...", "DEL ..."
        }
        if let Some((_, last_score)) = items.last() {
            // last_score is f64; convert to i64 millis then Crockford32
            let millis = *last_score as i64;
            next_cursor = encode_crockford32(millis);
        }
    }

    Json(EventsList {
        cursor: next_cursor,
        events,
    })
}

pub fn routes() -> Router<AppState> {
    Router::new().route(EVENTS_ROUTE, get(get_events_handler))
}

#[derive(OpenApi)]
#[openapi(paths(get_events_handler), components(schemas(EventsList)))]
pub struct EventsApiDoc;
