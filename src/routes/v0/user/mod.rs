use crate::register_routes;
use crate::routes::v0::endpoints;
use crate::routes::AppState;
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
    register_routes!(Router::new(),
        endpoints::USER_ROUTE => view::user_view_handler,
        endpoints::USER_DETAILS_ROUTE => details::user_details_handler,
        endpoints::RELATIONSHIP_ROUTE => relationship::user_relationship_handler,
        endpoints::USER_TAGS_ROUTE => tags::user_tags_handler,
        endpoints::USER_TAGGERS_ROUTE => tags::user_taggers_handler,
        endpoints::USER_COUNTS_ROUTE => counts::user_counts_handler,
        endpoints::USER_FOLLOWERS_ROUTE => follows::user_followers_handler,
        endpoints::USER_FOLLOWING_ROUTE => follows::user_following_handler,
        endpoints::USER_FRIENDS_ROUTE => follows::user_friends_handler,
        endpoints::USER_MUTED_ROUTE => muted::user_muted_handler,
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
        combined.merge(muted::UserMutedApiDoc::openapi());
        combined
    }
}
