//! The API, served under every version prefix from one declaration.
//!
//! Every module declares its routes and OpenAPI paths once, relative to the prefix
//! (`endpoints`). [`routes`] mounts them under `/v0` and `/v1`, and [`ApiDoc`] prefixes the one
//! document the same way, so a route cannot exist in one version and not the other by
//! accident. The two versions answer identically for now: the differences a `social/v1` client
//! needs are carved out as types, never as a second handler or route declaration.

use crate::models::{BoundedLimit, BoundedSkip};
use axum::Router;
use endpoints::{V0_PREFIX, V1_PREFIX, VERSION_PREFIXES};
use utoipa::OpenApi;

pub mod bootstrap;
pub mod endpoints;
pub mod events;
pub mod file;
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

/// Returns (expensive_routes, default_routes), each mounted under every version prefix.
/// Expensive routes receive tighter rate limiting. Both versions of a bucket come back in one
/// router so that one rate-limit layer, and so one bucket per client, covers them both.
pub fn routes(app_state: AppState) -> (Router<AppState>, Router<AppState>) {
    let (expensive, default) = relative_routes(app_state);
    (
        mount_under_every_version(expensive),
        mount_under_every_version(default),
    )
}

/// `router` under each of [`VERSION_PREFIXES`]. Axum flattens a nested router into the outer
/// one, route by route, so two buckets nested under the same prefix still merge.
fn mount_under_every_version(router: Router<AppState>) -> Router<AppState> {
    VERSION_PREFIXES
        .iter()
        .fold(Router::new(), |mounted, prefix| {
            mounted.nest(prefix, router.clone())
        })
}

/// The buckets as the modules declare them, with paths relative to the version prefix.
fn relative_routes(app_state: AppState) -> (Router<AppState>, Router<AppState>) {
    let expensive = Router::new()
        .merge(stream::expensive_routes())
        .merge(tag::expensive_routes())
        .merge(search::expensive_routes())
        .merge(file::expensive_routes())
        .merge(bootstrap::expensive_routes());

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
    BoundedLimit<40, 40>,
    BoundedLimit<40, 100>,
    BoundedLimit<50, 200>,
    BoundedLimit<500, 1000>,
    BoundedSkip<1000>,
    BoundedSkip<10_000>
)))]
pub struct ApiDoc;

impl ApiDoc {
    /// The `/v0` document: the frozen API.
    pub fn v0_docs() -> utoipa::openapi::OpenApi {
        Self::docs_under(V0_PREFIX)
    }

    /// The `/v1` document, for the `social/v1` clients.
    pub fn v1_docs() -> utoipa::openapi::OpenApi {
        Self::docs_under(V1_PREFIX)
    }

    /// The one document with every path under `prefix`, as [`routes`] mounts them. Nesting
    /// prefixes the paths and carries the schemas over, so both documents list the same
    /// operations and components.
    fn docs_under(prefix: &str) -> utoipa::openapi::OpenApi {
        Self::openapi().nest(prefix, Self::relative_docs())
    }

    /// Every module's document merged, with paths relative to the version prefix.
    fn relative_docs() -> utoipa::openapi::OpenApi {
        let mut combined = post::PostApiDoc::merge_docs();
        combined.merge(bootstrap::BootstrapApiDoc::openapi());
        combined.merge(info::InfoApiDoc::openapi());
        combined.merge(user::UserApiDoc::merge_docs());
        combined.merge(stream::StreamApiDoc::merge_docs());
        combined.merge(search::SearchApiDoc::merge_docs());
        combined.merge(file::FileApiDoc::merge_docs());
        combined.merge(tag::TagApiDoc::merge_docs());
        combined.merge(resource::ResourceApiDoc::openapi());
        combined.merge(notification::NotificationApiDoc::merge_docs());
        combined.merge(events::EventsApiDoc::openapi());

        combined
    }
}
