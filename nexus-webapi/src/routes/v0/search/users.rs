use crate::models::{
    BoundedLimit, BoundedPagination, BoundedSkip, Tags, UserIdPrefix, UsernamePrefix,
};
use crate::routes::v0::endpoints::{
    SEARCH_USERS_BY_ID_ROUTE, SEARCH_USERS_BY_NAME_ROUTE, SEARCH_USERS_BY_TAGS_ROUTE,
};
use crate::routes::v0::search::USER_ID_SEARCH_MIN_PREFIX_LEN;
use crate::routes::Path;
use crate::routes::Query;
use crate::Result;
use axum::Json;
use nexus_common::models::user::{UserSearch, UsersByTagSearch};
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

#[derive(Deserialize)]
pub struct SearchUsersByTagsQuery {
    pub tags: Tags,
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 200>,
}

#[utoipa::path(
    get,
    path = SEARCH_USERS_BY_TAGS_ROUTE,
    description = "Search users by profile tags, scored by how many taggers applied the searched labels. Equal scores break ties by user id descending",
    tag = "Search",
    params(
        ("tags" = Tags, Query, description = "Comma-separated tag labels (1-5). Users tagged with any of them are returned"),
        ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N results (max 10000)"),
        ("limit" = Option<BoundedLimit<20, 200>>, Query, description = "Limit the number of results (1-200, default 20)")
    ),
    responses(
        (status = 200, description = "Search results ordered by tagger count", body = Vec<UsersByTagSearch>),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn search_users_by_tags_handler(
    Query(query): Query<SearchUsersByTagsQuery>,
) -> Result<Json<Vec<UsersByTagSearch>>> {
    debug!(
        "GET {SEARCH_USERS_BY_TAGS_ROUTE} tags:{:?}, skip: {}, limit: {}",
        query.tags,
        query.pagination.skip_value(),
        query.pagination.limit_value()
    );

    let pagination = query.pagination.to_pagination(None, None);

    let users = UsersByTagSearch::get_by_labels(&query.tags.to_string_vec(), pagination).await?;
    Ok(Json(users))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        search_users_by_name_handler,
        search_users_by_id_handler,
        search_users_by_tags_handler
    ),
    components(schemas(UserSearch, UsersByTagSearch, Tags, UsernamePrefix, UserIdPrefix))
)]
pub struct SearchUsersApiDocs;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_query(
        s: &str,
    ) -> std::result::Result<SearchUsersByTagsQuery, serde_urlencoded::de::Error> {
        serde_urlencoded::from_str(s)
    }

    #[test]
    fn tags_csv_parses_and_sanitizes() {
        let q = parse_query("tags=Dev,%20Free%20").expect("valid tags must parse");
        assert_eq!(q.tags.to_string_vec(), vec!["dev", "free"]);
    }

    #[test]
    fn tags_single_label_parses() {
        let q = parse_query("tags=synonym").expect("single tag must parse");
        assert_eq!(q.tags.to_string_vec(), vec!["synonym"]);
    }

    #[test]
    fn tags_missing_rejected() {
        assert!(parse_query("skip=0").is_err());
    }

    #[test]
    fn tags_over_five_rejected() {
        assert!(parse_query("tags=a,b,c,d,e,f").is_err());
    }

    #[test]
    fn tags_over_length_label_rejected() {
        let over_length = "a".repeat(21);
        assert!(parse_query(&format!("tags={over_length}")).is_err());
    }
}
