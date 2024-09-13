use crate::register_routes;
use crate::routes::v0::endpoints;
use axum::Router;
use utoipa::OpenApi;

mod details;
mod list;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        endpoints::FILE_ROUTE => details::file_details_handler,
        endpoints::FILE_LIST_ROUTE => list::file_details_by_uris_handler,
    )
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
