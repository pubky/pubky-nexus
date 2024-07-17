use axum::{routing::get_service, Router};
use std::env;
use tower_http::services::ServeDir;

pub fn routes() -> Router {
    let static_path = env::var("STATIC_PATH").expect("STATIC_PATH environment variable not set");
    Router::new().nest_service("/static", get_service(ServeDir::new(static_path)))
}
