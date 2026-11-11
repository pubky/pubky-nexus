//! `GET /v1/user/{user_id}/reach-set`, the users a user's Web of Trust reaches, for a client
//! that filters its own data by them (Pinakes' `wot_2` reach). `/v0` is frozen, so the route is
//! mounted and documented with `/v1` alone (`V1::own_routes`, the v1 document).

use crate::models::PubkyId;
use crate::routes::v0::endpoints::USER_REACH_SET_ROUTE;
use crate::routes::v0::stream::posts::resolve_wot_depth;
use crate::routes::{Path, Query};
use crate::{Error, Result};
use axum::http::header::{ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use nexus_common::models::follow::reach::{reach_set, ReachSet};
use serde::Deserialize;
use tracing::debug;

/// The most users a reach set lists; a longer reach is answered `truncated`.
pub const REACH_SET_MAX_USERS: usize = 1000;

#[derive(Deserialize)]
pub struct ReachSetQuery {
    depth: Option<u8>,
}

#[utoipa::path(
    get,
    path = USER_REACH_SET_ROUTE,
    tag = "User",
    description = "The users `user_id`'s Web of Trust reaches within `depth` follows: the authors a `source=wot` post stream reads, whether or not they posted, never `user_id` itself. The users `user_id` follows come first, then the rest, each by id; at most 1000, with `truncated` set when the reach holds more. The `ETag` names the set: send it back in `If-None-Match` and an unchanged set is a 304.",
    params(
        ("user_id" = String, Path, description = "User Pubky ID"),
        ("depth" = Option<u8>, Query, description = "WoT traversal depth, 1-3, default 2, as for `source=wot`"),
        ("If-None-Match" = Option<String>, Header, description = "The `ETag` of a set held: a 304 while it still matches")
    ),
    responses(
        (status = 200, description = "The reach set", body = ReachSet,
            headers(("ETag" = String, description = "Weak entity tag of the set"))),
        (status = 304, description = "The set still matches the `If-None-Match` tag",
            headers(("ETag" = String, description = "Weak entity tag of the set"))),
        (status = 400, description = "Invalid user ID or depth"),
        (status = 404, description = "User not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn user_reach_set_handler(
    Path(user_id): Path<PubkyId>,
    Query(query): Query<ReachSetQuery>,
    headers: HeaderMap,
) -> Result<Response> {
    debug!(
        "GET {USER_REACH_SET_ROUTE} user_id:{}, depth:{:?}",
        user_id, query.depth
    );
    let depth = resolve_wot_depth(query.depth)?;
    let Some(set) = reach_set(&user_id, depth, REACH_SET_MAX_USERS).await? else {
        return Err(Error::user_not_found(user_id));
    };
    let etag = set.etag();
    let tag = HeaderValue::from_str(&etag)?;
    if none_match(headers.get_all(IF_NONE_MATCH), &etag) {
        return Ok((StatusCode::NOT_MODIFIED, [(ETAG, tag)]).into_response());
    }
    Ok(([(ETAG, tag)], Json(set)).into_response())
}

/// Whether the `If-None-Match` fields, one list together (RFC 9110 5.3), name `etag` (weakly
/// compared, 13.1.2) or are `*`.
fn none_match<'a>(fields: impl IntoIterator<Item = &'a HeaderValue>, etag: &str) -> bool {
    fn opaque(tag: &str) -> &str {
        tag.strip_prefix("W/").unwrap_or(tag)
    }
    let etag = opaque(etag);
    fields
        .into_iter()
        .filter_map(|field| field.to_str().ok())
        .flat_map(|list| list.split(','))
        .map(str::trim)
        .any(|tag| tag == "*" || opaque(tag) == etag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_match_reads_the_tag_list() {
        let etag = "W/\"abc\"";
        let fields = |values: &[&'static str]| -> Vec<HeaderValue> {
            values
                .iter()
                .copied()
                .map(HeaderValue::from_static)
                .collect()
        };
        for values in [
            &["\"abc\""][..],
            &["W/\"abc\""],
            &["\"x\", \"abc\""],
            &["\"x\",W/\"abc\""],
            &["*"],
            &["\"x\"", "W/\"abc\""],
        ] {
            assert!(none_match(&fields(values), etag), "{values:?}");
        }
        for values in [
            &["\"abcd\""][..],
            &["abc"],
            &["W/\"x\""],
            &[""],
            &["\"x\"", "\"y\""],
        ] {
            assert!(!none_match(&fields(values), etag), "{values:?}");
        }
        assert!(!none_match(&fields(&[]), etag));
    }
}
