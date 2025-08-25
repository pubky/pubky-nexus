use crate::routes::AppState;

use super::endpoints::EVENTS_ROUTE;
use nexus_common::models::events::EventsList;

use axum::extract::Query;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

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
    // TODO: (524)
    // Example:
    //
    // PUT pubky://ujikizdojcuwn3m7nfypzt1xwiz8t68rqr9qbdbagmgotwd1duho/pub/pubky.app/profile.json
    // ...
    // DEL pubky://ujikizdojcuwn3m7nfypzt1xwiz8t68rqr9qbdbagmgotwd1duho/pub/pubky.app/settings
    // cursor: 0032W24REPR5W
    //
    // XXX: not JSON but text
    Json({})
}

pub fn routes() -> Router<AppState> {
    Router::new().route(EVENTS_ROUTE, get(get_events_handler))
}

#[derive(OpenApi)]
#[openapi(paths(get_events_handler), components(schemas(EventsList)))]
pub struct EventsApiDoc;
