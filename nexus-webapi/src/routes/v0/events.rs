use crate::routes::AppState;
use nexus_common::db::{kv::SortOrder, RedisOps};
use nexus_common::models::event::Event;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::endpoints::EVENTS_ROUTE;
use crockford;

use crate::Error;
use axum::extract::Query;
// use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EventsList {
    cursor: String,
    events: Vec<String>,
}

#[derive(Deserialize)]
pub struct EventsQuery {
    cursor: Option<String>,
    limit: Option<usize>,
}

fn encode_crockford32(ts_ms: i64) -> String {
    crockford::encode(ts_ms as u64)
}
// HACK: return proper error?
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
pub async fn get_events_handler(Query(q): Query<EventsQuery>) -> Result<Json<EventsList>, Error> {
    let (limit, cursor) = parse_query(&q)?;
    let items = get_from_redis(cursor, limit).await?;
    let event_list = assemble_page(items);

    Ok(Json(event_list))
}

fn assemble_page(items: Vec<(String, f64)>) -> EventsList {
    let mut events = Vec::with_capacity(items.len());
    let mut cursor = "0000000000000".to_string();
    if !items.is_empty() {
        for (line, _score) in &items {
            events.push(line.clone()); // "PUT ...", "DEL ..."
        }
        if let Some((_, last_score)) = items.last() {
            // last_score is f64; convert to i64 millis then Crockford32
            let millis = *last_score as i64;
            cursor = encode_crockford32(millis);
        }
    }

    EventsList { events, cursor }
}

async fn get_from_redis(cursor: Option<f64>, limit: usize) -> Result<Vec<(String, f64)>, Error> {
    let result = Event::try_from_index_sorted_set(
        &["Events"],
        cursor,      // start (exclusive/inclusive is handled by the ZRANGEBYSCORE impl)
        None,        // end
        None,        // skip
        Some(limit), // limit
        SortOrder::Ascending,
        None, // prefix -> "Sorted"
    )
    .await;

    let result = match result {
        Ok(r) => r,
        Err(source) => return Err(Error::InternalServerError { source }),
    };

    match result {
        Some(v) => Ok(v),
        None => Ok(Vec::new()),
    }
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
