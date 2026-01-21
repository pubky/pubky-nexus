use crate::routes::v0::endpoints::{SEARCH_USERS_BY_ID_ROUTE, SEARCH_USERS_BY_NAME_ROUTE};
use crate::routes::v0::search::USER_ID_SEARCH_MIN_PREFIX_LEN;
use crate::{Error, Result};
use axum::extract::{Path, Query};
use axum::Json;
use nexus_common::models::user::UserSearch;
use nexus_common::types::Pagination;
use serde::Deserialize;
use tracing::debug;
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

    debug!("GET {SEARCH_USERS_BY_NAME_ROUTE} username:{}", username);

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(200);

    match UserSearch::get_by_name(&username, Some(skip), Some(limit)).await? {
        Some(user_search) => Ok(Json(user_search)),
        None => Ok(Json(UserSearch::default())),
    }
}

#[utoipa::path(
    get,
    path = SEARCH_USERS_BY_ID_ROUTE,
    description = "Search user IDs by ID prefix",
    tag = "Search",
    params(
        ("prefix" = String, Path, description = format!("User ID prefix to search for (at least {USER_ID_SEARCH_MIN_PREFIX_LEN} characters)")),
        ("skip" = Option<usize>, Query, description = "Skip N results"),
        ("limit" = Option<usize>, Query, description = "Limit the number of results")
    ),
    responses(
        (status = 200, description = "Search results", body = UserSearch),
        (status = 400, description = "Invalid input"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_users_by_id_handler(
    Path(prefix): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<UserSearch>> {
    let id_prefix = prefix;
    if id_prefix.trim().chars().count() < USER_ID_SEARCH_MIN_PREFIX_LEN {
        return Err(Error::invalid_input(&format!(
            "ID prefix must be at least {USER_ID_SEARCH_MIN_PREFIX_LEN} chars"
        )));
    }

    debug!("GET {SEARCH_USERS_BY_ID_ROUTE} ID:{}", id_prefix);

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(200);

    match UserSearch::get_by_id(&id_prefix, Some(skip), Some(limit)).await? {
        Some(user_search) => Ok(Json(user_search)),
        None => Ok(Json(UserSearch::default())),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(search_users_by_name_handler, search_users_by_id_handler),
    components(schemas(UserSearch))
)]
pub struct SearchUsersApiDocs;
