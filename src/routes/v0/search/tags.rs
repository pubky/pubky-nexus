use crate::models::post::PostStream;
use crate::models::tag::search::TagSearch;
use crate::models::user::UserSearch;
use crate::routes::v0::endpoints::SEARCH_TAGS_ROUTE;
use crate::routes::v0::queries::PostStreamQuery;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use log::info;
use utoipa::OpenApi;

#[utoipa::path(
    get,
    path = SEARCH_TAGS_ROUTE,
    tag = "Search Users",
    params(
        ("viewer_id" = Option<String>, Query, description = "Viewer Pubky ID"),
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
    Query(query): Query<PostStreamQuery>,
) -> Result<Json<PostStream>> {
    info!(
        "GET {SEARCH_TAGS_ROUTE} label:{}, sort_by: {:?}, viewer_id: {:?}, skip: {:?}, limit: {:?}",
        label, query.sorting, query.viewer_id, query.skip, query.limit
    );

    match TagSearch::get_by_label(
        &label,
        query.sorting,
        query.viewer_id,
        query.skip,
        query.limit,
    )
    .await
    {
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
