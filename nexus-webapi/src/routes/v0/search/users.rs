use crate::models::{BoundedLimit, BoundedPagination, BoundedSkip, UserIdPrefix, UsernamePrefix};
use crate::routes::v0::endpoints::{SEARCH_USERS_BY_ID_ROUTE, SEARCH_USERS_BY_NAME_ROUTE};
use crate::routes::v0::search::USER_ID_SEARCH_MIN_PREFIX_LEN;
use crate::routes::Path;
use crate::routes::Query;
use crate::Result;
use axum::Json;
use nexus_common::models::user::UserSearch;
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchQuery {
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 50, 200>,
}

#[utoipa::path(
    get,
    path = SEARCH_USERS_BY_NAME_ROUTE,
    description = "Search user id by username prefix",
    tag = "Search",
    params(
        ("prefix" = UsernamePrefix, Path, description = "Username prefix to search for"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N results (max 10000)"),
        ("limit" = Option<BoundedLimit<50, 200>>, Query, description = "Limit the number of results (1–200, default 50)")
    ),
    responses(
        (status = 200, description = "Search results", body = UserSearch),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_users_by_name_handler(
    Path(prefix): Path<UsernamePrefix>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<UserSearch>> {
    debug!("GET {SEARCH_USERS_BY_NAME_ROUTE} username:{}", prefix);

    let pagination = query.pagination.to_pagination(None, None);

    match UserSearch::get_by_name(&prefix, pagination.skip, pagination.limit).await? {
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
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N results (max 10000)"),
        ("limit" = Option<BoundedLimit<50, 200>>, Query, description = "Limit the number of results (1–200, default 50)")
    ),
    responses(
        (status = 200, description = "Search results", body = UserSearch),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_users_by_id_handler(
    Path(prefix): Path<UserIdPrefix>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<UserSearch>> {
    debug!("GET {SEARCH_USERS_BY_ID_ROUTE} ID:{}", prefix);

    let pagination = query.pagination.to_pagination(None, None);

    match UserSearch::get_by_id(&prefix, pagination.skip, pagination.limit).await? {
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
