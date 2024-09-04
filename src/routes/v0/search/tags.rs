use crate::models::tag::search::TagSearch;
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
    tag = "Search Post by Tags",
    params(
        ("sorting" = Option<PostStreamSorting>, Query, description = "Sorting method"),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = Vec<TagSearch>),
        (status = 404, description = "No posts with that tag found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_post_tags_handler(
    Path(label): Path<String>,
    Query(query): Query<PostStreamQuery>,
) -> Result<Json<Vec<TagSearch>>> {
    info!(
        "GET {SEARCH_TAGS_ROUTE} label:{}, sort_by: {:?}, skip: {:?}, limit: {:?}",
        label, query.sorting, query.skip, query.limit
    );

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(20);

    match TagSearch::get_by_label(
        &label,
        query.sorting,
        skip,
        limit,
    )
    .await
    {
        Ok(Some(posts_list)) => Ok(Json(posts_list)),
        Ok(None) => Err(Error::PostNotFound {
            author_id: String::from("global"),
            post_id: String::from("N/A"),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(search_post_tags_handler), components(schemas(TagSearch)))]
pub struct SearchTagPostsApiDocs;
