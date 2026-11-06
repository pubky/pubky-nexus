use crate::routes::v0::endpoints::{
    RELATIONSHIP_ROUTE, USER_COUNTS_ROUTE, USER_DETAILS_ROUTE, USER_FOLLOWERS_ROUTE,
    USER_FOLLOWING_ROUTE, USER_FRIENDS_ROUTE, USER_ROUTE, USER_TAGGERS_ROUTE, USER_TAGS_ROUTE,
};
use crate::routes::v0::version::ApiVersion;
use crate::routes::AppState;

use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

pub mod counts;
mod details;
mod follows;
mod relationship;
pub mod tags;
pub mod view;

/// The user routes for one version: the view, the counts, the tags and the taggers in the
/// version's shape, the taggers with the version's label rule, the rest shared.
pub fn routes<V: ApiVersion>() -> Router<AppState> {
    Router::new()
        .route(USER_ROUTE, V::user_view())
        .route(USER_DETAILS_ROUTE, get(details::user_details_handler))
        .route(
            RELATIONSHIP_ROUTE,
            get(relationship::user_relationship_handler),
        )
        .route(USER_TAGS_ROUTE, V::user_tags())
        .route(USER_TAGGERS_ROUTE, V::user_taggers())
        .route(USER_COUNTS_ROUTE, V::user_counts())
        .route(USER_FOLLOWERS_ROUTE, get(follows::user_followers_handler))
        .route(USER_FOLLOWING_ROUTE, get(follows::user_following_handler))
        .route(USER_FRIENDS_ROUTE, get(follows::user_friends_handler))
}

#[derive(OpenApi)]
#[openapi()]
pub struct UserApiDoc;

impl UserApiDoc {
    /// The shared user routes and schemas; the versioned view, counts, tags and taggers come
    /// with the version's document.
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = view::UserViewApiDoc::openapi();
        combined.merge(counts::UserCountsApiDoc::openapi());
        combined.merge(details::UserDetailsApiDoc::openapi());
        combined.merge(relationship::RelationshipApiDoc::openapi());
        combined.merge(tags::UserTagsApiDoc::openapi());
        combined.merge(follows::UserFollowsApiDoc::openapi());
        combined
    }
}
