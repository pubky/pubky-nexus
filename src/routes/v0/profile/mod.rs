use axum::Router;
use utoipa::OpenApi;

mod counts;
mod details;
mod full;
mod relationship;
mod tags;

pub fn routes() -> Router {
    let counts_route = counts::route();
    let details_route = details::route();
    let full_view_route = full::route();
    let relationship_route = relationship::route();
    let tags_route = tags::route();

    full_view_route
        .merge(counts_route)
        .merge(details_route)
        .merge(relationship_route)
        .merge(tags_route)
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
