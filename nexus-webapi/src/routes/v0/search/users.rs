use crate::models::{
    BoundedLimit, BoundedPagination, BoundedSkip, PubkyId, TagLabels, UserIdPrefix, UsernamePrefix,
};
use crate::routes::v0::endpoints::{
    SEARCH_USERS_BY_ID_ROUTE, SEARCH_USERS_BY_NAME_ROUTE, SEARCH_USERS_BY_TAGS_ROUTE,
};
use crate::routes::v0::search::USER_ID_SEARCH_MIN_PREFIX_LEN;
use crate::routes::v0::version::{versioned_handlers, ApiVersion};
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::Json;
use nexus_common::models::user::{UserSearch, UsersByTagSearch};
use nexus_common::types::StreamReach;
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

/// The query of `GET /search/users/by_tags`, with the labels of `tags` folded by version `V`'s
/// rule. Every other parameter is the same on both.
#[derive(Deserialize)]
#[serde(bound(deserialize = ""))]
pub struct SearchUsersByTagsQuery<V: ApiVersion> {
    pub tags: V::Tags,
    pub user_id: Option<PubkyId>,
    pub reach: Option<StreamReach>,
    #[serde(flatten)]
    pub pagination: BoundedPagination<10_000, 20, 200>,
}

/// The one body behind `search_users_by_tags_handler_v0` and `search_users_by_tags_handler_v1`:
/// the users tagged with any of the labels, as the version folds them.
pub async fn search_users_by_tags<V: ApiVersion>(
    Query(query): Query<SearchUsersByTagsQuery<V>>,
) -> Result<Json<Vec<UsersByTagSearch>>> {
    debug!(
        "GET {SEARCH_USERS_BY_TAGS_ROUTE} tags:{:?}, user_id: {:?}, reach: {:?}, skip: {}, limit: {}",
        query.tags,
        query.user_id,
        query.reach,
        query.pagination.skip_value(),
        query.pagination.limit_value()
    );

    if query.user_id.is_some() ^ query.reach.is_some() {
        return Err(Error::invalid_input(
            "user_id and reach should be both provided together",
        ));
    }

    let pagination = query.pagination.to_pagination(None, None);
    let labels = query.tags.to_string_vec();

    let users = match (query.user_id, query.reach) {
        (Some(user_id), Some(reach)) => {
            UsersByTagSearch::get_by_labels_with_reach(&labels, &user_id, reach, pagination).await?
        }
        _ => UsersByTagSearch::get_by_labels(&labels, pagination).await?,
    };
    Ok(Json(users))
}

versioned_handlers! {
    search_users_by_tags_handler_v0 / search_users_by_tags_handler_v1 => search_users_by_tags(query: Query<shapes::SearchUsersByTagsQuery>) -> Vec<UsersByTagSearch>;
    #[utoipa::path(
        get,
        path = SEARCH_USERS_BY_TAGS_ROUTE,
        operation_id = "search_users_by_tags_handler",
        description = "Search users by profile tags, scored by how many taggers applied the searched labels. Equal scores break ties by user id descending. With `user_id` and `reach`, only users in that reach are returned (never `user_id` itself), with unchanged scores",
        tag = "Search",
        params(
            ("tags" = shapes::Tags, Query, description = "Comma-separated tag labels (1-5). Users tagged with any of them are returned"),
            ("user_id" = Option<PubkyId>, Query, description = "User ID to base reach on. Must be provided together with reach"),
            ("reach" = Option<StreamReach>, Query, example = "wot_2", description = "Reach type: `followers` | `following` | `friends` | `wot` | `wot_1`..`wot_3`. To apply that, user_id is required. Bare `wot` defaults to depth 2."),
            ("skip" = Option<BoundedSkip<10_000>>, Query, description = "Skip N results (max 10000)"),
            ("limit" = Option<BoundedLimit<20, 200>>, Query, description = "Limit the number of results (1-200, default 20)")
        )
    )]
    responses(
        (status = 200, description = "Search results ordered by tagger count"),
        (status = 400, description = "Invalid parameters"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
}

/// The user searches by name and by id and the schemas the user searches share; the search by
/// tags, whose label list is the version's, comes with the version's document.
#[derive(OpenApi)]
#[openapi(
    paths(search_users_by_name_handler, search_users_by_id_handler),
    components(schemas(
        UserSearch,
        UsersByTagSearch,
        UsernamePrefix,
        UserIdPrefix,
        StreamReach
    ))
)]
pub struct SearchUsersApiDocs;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::v0::version::{V0, V1};

    fn parse_query(
        s: &str,
    ) -> std::result::Result<SearchUsersByTagsQuery<V0>, serde_urlencoded::de::Error> {
        serde_urlencoded::from_str(s)
    }

    #[test]
    fn tags_csv_parses_and_sanitizes() {
        let q = parse_query("tags=Dev,%20Free%20").expect("valid tags must parse");
        assert_eq!(q.tags.to_string_vec(), vec!["dev", "free"]);
    }

    /// On `/v1` the list folds by the `social/v1` rule: ASCII case only, so a non-ASCII
    /// uppercase letter stays, where `/v0` lowercases it.
    #[test]
    fn v1_tags_csv_folds_by_the_v1_rule() {
        let v1: SearchUsersByTagsQuery<V1> =
            serde_urlencoded::from_str("tags=Dev,%20Free%20,%C3%84rger").unwrap();
        assert_eq!(v1.tags.to_string_vec(), vec!["dev", "free", "Ärger"]);
        let v0 = parse_query("tags=Dev,%20Free%20,%C3%84rger").unwrap();
        assert_eq!(v0.tags.to_string_vec(), vec!["dev", "free", "ärger"]);
        assert!(
            serde_urlencoded::from_str::<SearchUsersByTagsQuery<V1>>("tags=dev,co%20ol").is_err()
        );
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
