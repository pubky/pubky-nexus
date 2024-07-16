pub mod info;

use axum::routing::get;
use axum::Router;

pub fn create_routes() -> Router {
    Router::new().route("/v0/info", get(info::info_handler))
}
