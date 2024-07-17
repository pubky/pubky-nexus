pub mod endpoints;
pub mod info;

use axum::routing::get;
use axum::Router;
use endpoints::INFO_PATH;

pub fn create_routes() -> Router {
    Router::new().route(INFO_PATH, get(info::info_handler))
}
