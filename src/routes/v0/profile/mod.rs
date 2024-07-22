use crate::register_routes;
use crate::routes::v0::endpoints;
use axum::Router;
use utoipa::OpenApi;

mod counts;
mod details;
mod full;
mod relationship;
mod tags;

pub fn routes() -> Router {
    register_routes!(Router::new(),
        endpoints::PROFILE_ROUTE => full::profile_full_view_handler,
        endpoints::PROFILE_DETAILS_ROUTE => details::profile_details_handler,
        endpoints::RELATIONSHIP_ROUTE => relationship::profile_relationship_handler,
        endpoints::PROFILE_TAGS_ROUTE => tags::profile_tags_handler,
        endpoints::PROFILE_COUNTS_ROUTE => counts::profile_counts_handler,
    )
}

#[derive(OpenApi)]
#[openapi()]
pub struct ProfileApiDoc;

impl ProfileApiDoc {
    pub fn merge_docs() -> utoipa::openapi::OpenApi {
        let mut combined = full::ProfileViewApiDoc::openapi();
        combined.merge(counts::ProfileCountsApiDoc::openapi());
        combined.merge(details::ProfileDetailsApiDoc::openapi());
        combined.merge(relationship::RelationshipApiDoc::openapi());
        combined.merge(tags::ProfileTagsApiDoc::openapi());
        combined
    }
}
