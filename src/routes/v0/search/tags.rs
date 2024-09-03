use crate::models::post::PostStream;
use crate::models::tag::search::{SortBy, TagSearch};
use crate::models::user::UserSearch;
use crate::routes::v0::endpoints::SEARCH_TAGS_ROUTE;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchQuery {
    skip: Option<usize>,
    limit: Option<usize>,
    sort_by: Option<SortBy>,
}

#[utoipa::path(
    get,
    path = SEARCH_TAGS_ROUTE,
    tag = "Search Users",
    params(
        ("sort_by" = Option<SortBy>, Query, description = "Username to search for"),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = UserSearch),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "No users found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_post_tags_handler(
    Path(label): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<PostStream>> {
    //TODO: Maybe add viewer_id as a optional param
    info!(
        "GET {SEARCH_TAGS_ROUTE} label:{}, sort_by: {:?}, skip: {:?}, limit: {:?}",
        label, query.sort_by, query.skip, query.limit
    );

    match TagSearch::get_by_label(&label, query.sort_by, query.skip, query.limit).await {
        Ok(Some(stream)) => Ok(Json(stream)),
        Ok(None) => Err(Error::PostNotFound {
            author_id: String::from("global"),
            post_id: String::from("N/A"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(search_post_tags_handler), components(schemas(UserSearch)))]
pub struct SearchTagPostsApiDocs;
