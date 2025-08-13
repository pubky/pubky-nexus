use crate::routes::v0::endpoints::{
    RELATIONSHIP_ROUTE, USER_COUNTS_ROUTE, USER_DETAILS_ROUTE, USER_FOLLOWERS_ROUTE,
    USER_FOLLOWING_ROUTE, USER_FRIENDS_ROUTE, USER_MUTED_ROUTE, USER_ROUTE, USER_TAGGERS_ROUTE,
    USER_TAGS_ROUTE,
};
use crate::routes::AppState;

use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

mod counts;
mod details;
mod follows;
mod muted;
mod relationship;
pub mod tags;
mod view;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(USER_ROUTE, get(view::user_view_handler))
        .route(USER_DETAILS_ROUTE, get(details::user_details_handler))
        .route(
            RELATIONSHIP_ROUTE,
            get(relationship::user_relationship_handler),
        )
        .route(USER_TAGS_ROUTE, get(tags::user_tags_handler))
        .route(USER_TAGGERS_ROUTE, get(tags::user_taggers_handler))
        .route(USER_COUNTS_ROUTE, get(counts::user_counts_handler))
        .route(USER_FOLLOWERS_ROUTE, get(follows::user_followers_handler))
        .route(USER_FOLLOWING_ROUTE, get(follows::user_following_handler))
        .route(USER_FRIENDS_ROUTE, get(follows::user_friends_handler))
        .route(USER_MUTED_ROUTE, get(muted::user_muted_handler))
}

#[derive(OpenApi)]
#[openapi()]
pub struct UserApiDoc;

impl UserApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = view::UserViewApiDoc::openapi();
        combined.merge(counts::UserCountsApiDoc::openapi());
        combined.merge(details::UserDetailsApiDoc::openapi());
        combined.merge(relationship::RelationshipApiDoc::openapi());
        combined.merge(tags::UserTagsApiDoc::openapi());
        combined.merge(follows::UserFollowsApiDoc::openapi());
        combined.merge(muted::UserMutedApiDoc::openapi());
        combined
    }
}
