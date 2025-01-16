use crate::{
    models::file::details::FileVariant, static_processor::static_files_middleware, Config, Result,
};
use axum::{
    extract::Request,
    middleware::{self},
    response::{IntoResponse, Redirect, Response},
    routing::{self, get_service},
    Router,
};
use tower_http::services::ServeDir;

const STATIC_ROUTE: &str = "/static";
const STATIC_FILES_ROUTE: &str = "/static/files/{owner_id}/{file_id}/{variant}";
const LEGACY_STATIC_FILES_ROUTE: &str = "/static/files/{owner_id}/{file_id}";

pub fn routes() -> Router {
    let config = Config::from_env();

    let files = Router::new()
        .route_service(
            STATIC_FILES_ROUTE,
            get_service(ServeDir::new(config.file_path.clone())),
        )
        .route_layer(middleware::from_fn(static_files_middleware));

    let legacy_files = files.route_service(
        LEGACY_STATIC_FILES_ROUTE,
        routing::get(legacy_files_handler),
    );

    legacy_files.nest_service(STATIC_ROUTE, get_service(ServeDir::new(config.static_path)))
}

/// Handler to redirect legacy static files
/// The path should be in the format /static/{owner_id}/{file_id}
async fn legacy_files_handler(request: Request) -> Result<Response> {
    // Construct the new path
    let new_path = format!("{}/{}", request.uri().path(), FileVariant::Main);

    // Perform a redirect to the new path
    Ok(Redirect::permanent(&new_path).into_response())
}
