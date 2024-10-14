use crate::routes::v0::endpoints;
use crate::{register_routes, to_axum};
use axum::Router;
use utoipa::OpenApi;

mod counts;
mod details;
mod follows;
mod relationship;
mod tags;
mod view;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        to_axum!(endpoints::USER_ROUTE) => view::user_view_handler,
        to_axum!(endpoints::USER_DETAILS_ROUTE) => details::user_details_handler,
        to_axum!(endpoints::RELATIONSHIP_ROUTE) => relationship::user_relationship_handler,
        to_axum!(endpoints::USER_TAGS_ROUTE) => tags::user_tags_handler,
        to_axum!(endpoints::USER_TAGGERS_ROUTE) => tags::user_taggers_handler,
        to_axum!(endpoints::USER_COUNTS_ROUTE) => counts::user_counts_handler,
        to_axum!(endpoints::USER_FOLLOWERS_ROUTE) => follows::user_followers_handler,
        to_axum!(endpoints::USER_FOLLOWING_ROUTE) => follows::user_following_handler,
        to_axum!(endpoints::USER_FRIENDS_ROUTE) => follows::user_friends_handler,
    )
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
        combined
    }
}
