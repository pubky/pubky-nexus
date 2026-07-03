use crate::routes::v0::endpoints::{FILE_LIST_ROUTE, FILE_ROUTE};
use crate::routes::AppState;

use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

mod details;
pub mod list;

pub fn expensive_routes() -> Router<AppState> {
    Router::new().route(FILE_LIST_ROUTE, post(list::file_details_by_uris_handler))
}

pub fn routes() -> Router<AppState> {
    Router::new().route(FILE_ROUTE, get(details::file_details_handler))
}

#[derive(OpenApi)]
#[openapi()]
pub struct FileApiDoc;

impl FileApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = details::FileDetailsApiDoc::openapi();
        combined.merge(list::FilesListApiDoc::openapi());
        combined
    }
}
