use crate::models::{BoundedLimit, BoundedSkip};
use axum::Router;
use utoipa::OpenApi;

pub mod bootstrap;
pub mod endpoints;
pub mod events;
pub mod file;
pub mod graph;
pub mod info;
pub mod notification;
pub mod post;
pub mod resource;
pub mod search;
pub mod stream;
pub mod tag;
mod types;
pub mod user;

pub use types::{TaggersInfoResponse, TagsQuery};

use super::AppState;

/// Returns (expensive_routes, default_routes).
/// Expensive routes receive tighter rate limiting.
pub fn routes(app_state: AppState) -> (Router<AppState>, Router<AppState>) {
    let expensive = Router::new()
        .merge(stream::expensive_routes())
        .merge(tag::expensive_routes())
        .merge(search::expensive_routes())
        .merge(file::expensive_routes())
        .merge(bootstrap::expensive_routes())
        .merge(graph::routes());

    let default = Router::new()
        .merge(info::routes(app_state.clone()))
        .merge(post::routes())
        .merge(user::routes())
        .merge(stream::routes())
        .merge(search::routes())
        .merge(file::routes())
        .merge(tag::routes())
        .merge(resource::routes())
        .merge(notification::routes())
        .merge(bootstrap::routes())
        .merge(events::routes());

    (expensive, default)
}

#[derive(OpenApi)]
#[openapi(components(schemas(
    BoundedLimit<5, 20>,
    BoundedLimit<5, 100>,
    BoundedLimit<10, 50>,
    BoundedLimit<10, 100>,
    BoundedLimit<20, 20>,
    BoundedLimit<20, 100>,
    BoundedLimit<20, 200>,
    BoundedLimit<30, 50>,
    BoundedLimit<40, 40>,
    BoundedLimit<40, 100>,
    BoundedLimit<50, 200>,
    BoundedLimit<500, 1000>,
    BoundedSkip<1000>,
    BoundedSkip<10_000>
)))]
pub struct ApiDoc;

impl ApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = post::PostApiDoc::merge_docs();
        combined.merge(bootstrap::BootstrapApiDoc::openapi());
        combined.merge(info::InfoApiDoc::openapi());
        combined.merge(user::UserApiDoc::merge_docs());
        combined.merge(stream::StreamApiDoc::merge_docs());
        combined.merge(search::SearchApiDoc::merge_docs());
        combined.merge(file::FileApiDoc::merge_docs());
        combined.merge(graph::GraphApiDoc::openapi());
        combined.merge(tag::TagApiDoc::merge_docs());
        combined.merge(resource::ResourceApiDoc::openapi());
        combined.merge(notification::NotificationApiDoc::merge_docs());
        combined.merge(events::EventsApiDoc::openapi());
        combined.merge(ApiDoc::openapi());

        combined
    }
}
