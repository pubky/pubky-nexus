use crate::models::{BoundedLimit, BoundedSkip};
use axum::Router;
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

/// Returns (expensive_routes, default_routes).
/// Expensive routes receive tighter rate limiting.
pub fn routes(app_state: AppState) -> (Router<AppState>, Router<AppState>) {
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
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
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
        combined.merge(ApiDoc::openapi());

        let description = combined.info.description.take().unwrap_or_default();
        combined.info.description = Some(format!("{description}\n\n{LIGHT_MODE_GUIDE}"));
        combined
    }
}

/// How a client uses a light Nexus; appended to the API description shown in Swagger UI.
const LIGHT_MODE_GUIDE: &str = "\
## Light mode

A Nexus runs in `full` or `light` mode; `GET /v0/info` reports which in `mode`.

A **full** Nexus serves everything it indexes, content included.

A **light** Nexus serves the same social graph (follows, tags, replies, reposts, mentions, \
bookmarks, counts, streams) but none of the content people wrote or uploaded. A client \
fetches that content from each owner's homeserver:

- Posts come without `content`. Fetch the post at its `uri` from the author's homeserver, \
given in `author_homeserver` on post views. Reuse a copy you fetched while `content_hash` \
is unchanged.
- Users come without `name`, `bio`, `links` and `status`. Fetch the profile from the \
user's homeserver, given in `homeserver` on user views. Reuse a copy while \
`profile_hash` is unchanged.
- Files come as records without `name` or `urls`. Fetch `src` from its homeserver, \
unless `blocked` is `true`.
- A homeserver whose `status` is `unreachable` did not answer Nexus's last poll; one \
marked `stale` may no longer host the user, so confirm it through pkarr.

Endpoints that need content a light Nexus does not keep answer `501` with \
`{\"error\": \"unavailable in light mode\"}`: post content search, both name searches, \
the `collection` and `post_collections` post streams, and every `/static` route.";
