use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub mod endpoints;
pub mod info;
pub mod post;
pub mod user;

pub fn routes() -> Router {
    let routes_info = info::routes();
    let routes_post = post::routes();
    let route_user = user::routes();
    let route_openapi =
        SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::merge_docs());

    routes_post
        .merge(routes_info)
        .merge(route_user)
        .merge(route_openapi)
}

#[derive(OpenApi)]
#[openapi()]
pub struct ApiDoc;

impl ApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = post::PostApiDoc::merge_docs();
        combined.merge(info::InfoApiDoc::openapi());
        combined.merge(user::UserApiDoc::merge_docs());
        combined
    }
}
