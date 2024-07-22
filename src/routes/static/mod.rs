use axum::{routing::get_service, Router};
use tower_http::services::ServeDir;

pub fn routes(static_path: &String) -> Router {
    Router::new().nest_service("/static", get_service(ServeDir::new(static_path)))
}
