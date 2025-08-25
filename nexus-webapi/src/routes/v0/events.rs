use std::path::PathBuf;

use crate::routes::AppState;

use super::endpoints::EVENTS_ROUTE;
// TODO: create an Event view model
// use nexus_common::models::tag::view::TagView;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = EVENTS_ROUTE,
    tag = "Events",
    responses(
        // TODO: implement EventsList
        (status = 200, description = "Events list info", body = EventsList)
    )
)]
pub async fn events_handler(State(_app_state): State<AppState>) -> impl IntoResponse {
    // TODO:
}

// pub fn routes(app_state: AppState) -> Router<AppState> {
pub fn routes() -> Router<AppState> {
    Router::new()
        // .with_state(app_state)
        .route(EVENTS_ROUTE, get(events_handler))
}

#[derive(OpenApi)]
// TODO: implment EventsList
#[openapi(paths(events_handler), components(schemas(EventsList)))]
pub struct EventsApiDoc;
