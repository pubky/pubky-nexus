use crate::Config;
use axum::{routing::get_service, Router};
use tower_http::services::ServeDir;

pub fn routes() -> Router {
    let config = Config::from_env();
    Router::new().nest_service("/static", get_service(ServeDir::new(config.static_path)))
}
