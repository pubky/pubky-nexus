use axum::routing::get;
use axum::Router;
use endpoints::{
    INFO_ROUTE, PROFILE_COUNTS_ROUTE, PROFILE_DETAILS_ROUTE, PROFILE_ROUTE, PROFILE_TAGS_ROUTE,
    RELATIONSHIP_ROUTE,
};
use utoipa::OpenApi;

use crate::models::{
    info::ServerInfo,
    profile::{
        ProfileCounts, ProfileDetails, ProfileLink, ProfileTag, ProfileTags, ProfileView,
        Relationship,
    },
};

pub mod endpoints;
pub mod info;
pub mod profile;

pub fn routes() -> Router {
    Router::new()
        .route(INFO_ROUTE, get(info::info_handler))
        .route(PROFILE_ROUTE, get(profile::get_profile))
        .route(RELATIONSHIP_ROUTE, get(profile::get_relationship))
        .route(PROFILE_COUNTS_ROUTE, get(profile::get_counts))
        .route(PROFILE_DETAILS_ROUTE, get(profile::get_details))
        .route(PROFILE_TAGS_ROUTE, get(profile::get_tags))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        info::info_handler,
        profile::get_profile,
        profile::get_relationship,
        profile::get_counts,
        profile::get_details,
        profile::get_tags
    ),
    components(schemas(
        ServerInfo,
        ProfileView,
        ProfileCounts,
        ProfileDetails,
        ProfileTag,
        ProfileTags,
        Relationship,
        ProfileLink
    ))
)]
pub struct ApiDoc;
