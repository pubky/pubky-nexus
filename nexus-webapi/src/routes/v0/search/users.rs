use crate::models::{UserIdPrefix, UsernamePrefix};
use crate::routes::v0::endpoints::{SEARCH_USERS_BY_ID_ROUTE, SEARCH_USERS_BY_NAME_ROUTE};
use crate::routes::v0::search::USER_ID_SEARCH_MIN_PREFIX_LEN;
use crate::routes::Path;
use crate::Result;
use axum::extract::Query;
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
        ("prefix" = UsernamePrefix, Path, description = "Username prefix to search for"),
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
    Path(prefix): Path<UsernamePrefix>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<UserSearch>> {
    debug!("GET {SEARCH_USERS_BY_NAME_ROUTE} username:{}", prefix);

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(200);

    match UserSearch::get_by_name(prefix.as_str(), Some(skip), Some(limit)).await? {
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
        ("prefix" = UserIdPrefix, Path, description = format!("User ID prefix to search for (at least {USER_ID_SEARCH_MIN_PREFIX_LEN} characters)")),
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
    Path(prefix): Path<UserIdPrefix>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<UserSearch>> {
    debug!("GET {SEARCH_USERS_BY_ID_ROUTE} ID:{}", prefix);

    let skip = query.pagination.skip.unwrap_or(0);
    let limit = query.pagination.limit.unwrap_or(200);

    match UserSearch::get_by_id(prefix.as_str(), Some(skip), Some(limit)).await? {
        Some(user_search) => Ok(Json(user_search)),
        None => Ok(Json(UserSearch::default())),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(search_users_by_name_handler, search_users_by_id_handler),
    components(schemas(UserSearch, UsernamePrefix, UserIdPrefix))
)]
pub struct SearchUsersApiDocs;
