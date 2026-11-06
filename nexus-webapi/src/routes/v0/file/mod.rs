//! File details, the `File` nodes behind `/files`. Served and documented by `/v0` alone
//! (`V0::own_routes`, `V0Doc`): a `social/v1` file is bytes behind `/v1/static` with no
//! `File` node, so `/v1` has no file routes. The nodes, the Redis keys and these routes stay
//! for the `/v0` clients.

use crate::routes::v0::endpoints::{FILE_LIST_ROUTE, FILE_ROUTE};
use crate::routes::AppState;

use axum::routing::{get, post};
use axum::Router;

pub mod details;
pub mod list;

pub fn expensive_routes() -> Router<AppState> {
    Router::new().route(FILE_LIST_ROUTE, post(list::file_details_by_uris_handler))
}

pub fn routes() -> Router<AppState> {
    Router::new().route(FILE_ROUTE, get(details::file_details_handler))
}
