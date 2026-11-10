//! The API, served under every version prefix from one declaration.
//!
//! Every module declares its routes and OpenAPI paths once, relative to the prefix
//! (`endpoints`). [`routes`] builds them once per version and mounts each under its prefix,
//! and [`ApiDoc`] prefixes each version's document the same way, so a route cannot exist in one
//! version and not the other by accident. What a `social/v1` client reads differently is a type
//! of the version (`version`), never a second handler or route declaration, and a route only
//! one version serves is listed with that version (`ApiVersion::own_routes`).

use crate::models::{BoundedLimit, BoundedSkip};
use axum::Router;
use utoipa::OpenApi;
use version::{ApiVersion, V0, V1};

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
pub mod version;

pub use types::{TaggersInfoResponse, TagsQuery};

use super::AppState;

/// Returns (expensive_routes, default_routes), each holding every version's mount. Expensive
/// routes receive tighter rate limiting. Both versions of a bucket come back in one router so
/// that one rate-limit layer, and so one bucket per client, covers them both. Axum flattens a
/// nested router into the outer one, route by route, so two buckets nested under the same
/// prefix still merge.
pub fn routes(app_state: AppState) -> (Router<AppState>, Router<AppState>) {
    let (v0_expensive, v0_default) = relative_routes::<V0>(app_state.clone());
    let (v1_expensive, v1_default) = relative_routes::<V1>(app_state);
    (
        Router::new()
            .nest(V0::PREFIX, v0_expensive)
            .nest(V1::PREFIX, v1_expensive),
        Router::new()
            .nest(V0::PREFIX, v0_default)
            .nest(V1::PREFIX, v1_default),
    )
}

/// The buckets as the modules declare them for version `V`, with paths relative to its prefix,
/// each with the routes only `V` serves.
fn relative_routes<V: ApiVersion>(app_state: AppState) -> (Router<AppState>, Router<AppState>) {
    let (own_expensive, own_default) = V::own_routes();

    let expensive = Router::new()
        .merge(stream::expensive_routes::<V>())
        .merge(tag::expensive_routes())
        .merge(search::expensive_routes::<V>())
        .merge(bootstrap::expensive_routes::<V>())
        .merge(own_expensive);

    let default = Router::new()
        .merge(info::routes(app_state.clone()))
        .merge(post::routes::<V>())
        .merge(user::routes::<V>())
        .merge(stream::routes::<V>())
        .merge(search::routes::<V>())
        .merge(tag::routes::<V>())
        .merge(resource::routes::<V>())
        .merge(notification::routes::<V>())
        .merge(bootstrap::routes())
        .merge(events::routes())
        .merge(own_default);

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
        Self::docs_of::<V0>()
    }

    /// The `/v1` document, for the `social/v1` clients.
    pub fn v1_docs() -> utoipa::openapi::OpenApi {
        Self::docs_of::<V1>()
    }

    /// Version `V`'s document, with every path under its prefix as [`routes`] mounts them.
    /// Nesting prefixes the paths and carries the schemas over; the routes the version
    /// documents from outside its mount ([`ApiVersion::absolute_docs`]) are merged as they are.
    /// The description ends with the version's standing ([`ApiVersion::STANDING`]).
    fn docs_of<V: ApiVersion>() -> utoipa::openapi::OpenApi {
        let mut docs = Self::openapi()
            .nest(V::PREFIX, Self::relative_docs::<V>())
            .merge_from(V::absolute_docs());
        docs.info.description = Some(match &docs.info.description {
            Some(about) => format!("{about}\n\n{}", V::STANDING),
            None => V::STANDING.to_string(),
        });
        docs
    }

    /// Every module's document merged for version `V`, with paths relative to its prefix. The
    /// version's own document comes first: `merge` keeps the schema already present under a
    /// name, so the version's `PostKind` and the rest win over the stored models' schemas a
    /// shared route nests.
    fn relative_docs<V: ApiVersion>() -> utoipa::openapi::OpenApi {
        let mut combined = V::docs();
        combined.merge(post::PostApiDoc::merge_docs());
        combined.merge(bootstrap::BootstrapApiDoc::openapi());
        combined.merge(info::InfoApiDoc::openapi());
        combined.merge(user::UserApiDoc::merge_docs());
        combined.merge(stream::StreamApiDoc::merge_docs());
        combined.merge(search::SearchApiDoc::merge_docs());
        combined.merge(tag::TagApiDoc::merge_docs());
        combined.merge(resource::ResourceApiDoc::openapi());
        combined.merge(notification::NotificationApiDoc::merge_docs());
        combined.merge(events::EventsApiDoc::openapi());

        combined
    }
}
