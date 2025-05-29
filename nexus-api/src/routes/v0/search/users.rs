use crate::routes::v0::endpoints::SEARCH_USERS_BY_NAME_ROUTE;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::user::UserSearch;
use nexus_common::types::Pagination;
use serde::Deserialize;
use tracing::info;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(flatten)]
    pagination: Pagination,
}

#[utoipa::path(
    get,
    path = SEARCH_USERS_BY_NAME_ROUTE,
    description = "Search user id by username prefix",
    tag = "Search",
    params(
        ("prefix" = String, Path, description = "Username prefix to search for"),
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
pub async fn search_users_by_name_handler(
    Path(prefix): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<UserSearch>> {
    let username = prefix;
    if username.trim().is_empty() {
        return Err(Error::invalid_input("Username cannot be empty"));
    }

    info!("GET {SEARCH_USERS_BY_NAME_ROUTE} username:{}", username);

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(200);

    match UserSearch::get_by_name(&username, Some(skip), Some(limit)).await {
        Ok(Some(user_search)) => Ok(Json(user_search)),
        Ok(None) => Err(Error::UserNotFound {
            user_id: username.clone(),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(search_users_by_name_handler), components(schemas(UserSearch)))]
pub struct SearchUsersApiDocs;
