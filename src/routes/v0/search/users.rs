use crate::models::user::UserSearch;
use crate::routes::v0::endpoints::SEARCH_USERS_ROUTE;
use crate::{Error, Result};
use axum::extract::Query;
use axum::Json;
use log::info;
use serde::Deserialize;
use utoipa::OpenApi;

#[derive(Deserialize)]
pub struct SearchQuery {
    username: Option<String>,
    skip: Option<usize>,
    limit: Option<usize>,
}

#[utoipa::path(
    get,
    path = SEARCH_USERS_ROUTE,
    tag = "Search Users",
    params(
        ("username" = Option<String>, Query, description = "Username to search for"),
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
pub async fn search_users_handler(Query(query): Query<SearchQuery>) -> Result<Json<UserSearch>> {
    let username = match &query.username {
        Some(username) if !username.trim().is_empty() => username,
        _ => {
            return Err(Error::InvalidInput {
                message: "Username cannot be empty".to_string(),
            })
        }
    };

    info!("GET {SEARCH_USERS_ROUTE} username:{}", username);

    let skip = query.skip.unwrap_or(0);
    let limit = query.limit.unwrap_or(200);

    match UserSearch::get_by_name(username, Some(skip), Some(limit)).await {
        Ok(Some(user_search)) => Ok(Json(user_search)),
        Ok(None) => Err(Error::UserNotFound {
            user_id: username.clone(),
        }),
        Err(source) => Err(Error::InternalServerError { source }),
    }
}

#[derive(OpenApi)]
#[openapi(paths(search_users_handler), components(schemas(UserSearch)))]
pub struct SearchUsersApiDocs;
