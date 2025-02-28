use crate::register_routes;
use crate::routes::v0::endpoints;
use crate::routes::AppState;
use axum::Router;
use utoipa::OpenApi;

mod details;
mod list;

pub fn routes() -> Router<AppState> {
    let router = register_routes!(Router::new(),
        endpoints::FILE_ROUTE => details::file_details_handler,
    );
    router.route(
        endpoints::FILE_LIST_ROUTE,
        axum::routing::post(list::file_details_by_uris_handler),
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
