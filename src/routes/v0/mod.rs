use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod endpoints;
pub mod info;
pub mod post;
pub mod profile;
//pub mod tag;

pub fn routes() -> Router {
    let routes_info = info::routes();
    let routes_post = post::routes();
    let route_profile = profile::routes();
    let route_openapi =
        SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::merge_docs());

    routes_post
        .merge(routes_info)
        .merge(route_profile)
        .merge(route_openapi)
}

#[derive(OpenApi)]
#[openapi()]
pub struct ApiDoc;

impl ApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = post::PostApiDoc::openapi();
        combined.merge(info::InfoApiDoc::openapi());
        combined.merge(profile::ProfileApiDoc::merge_docs());
        combined
    }
}
