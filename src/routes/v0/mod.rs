use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod endpoints;
pub mod info;
pub mod profile;

pub fn routes() -> Router {
    let routes_info = info::routes();
    let route_profile = profile::routes();
    let route_openapi =
        SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::merge_docs());

    routes_info.merge(route_profile).merge(route_openapi)
}

#[derive(OpenApi)]
#[openapi()]
pub struct ApiDoc;

impl ApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = info::InfoApiDoc::openapi();
        combined.merge(profile::ProfileApiDoc::merge_docs());
        combined
    }
}
