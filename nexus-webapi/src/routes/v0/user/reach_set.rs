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

/// Whether the `If-None-Match` fields name `etag` or are `*` (RFC 9110 13.1.2). The fields are
/// one list (5.3): `*` matches only as the whole value; otherwise each field lists entity tags,
/// quoted strings that may hold commas, each optionally `W/`, compared weakly. A field that is
/// not such a list makes the header invalid, so it is ignored and the set is sent.
fn none_match<'a>(fields: impl IntoIterator<Item = &'a HeaderValue>, etag: &str) -> bool {
    let fields: Vec<&[u8]> = fields
        .into_iter()
        .map(|field| field.as_bytes().trim_ascii())
        .filter(|field| !field.is_empty())
        .collect();
    if fields == [b"*".as_slice()] {
        return true;
    }
    let own = entity_tags(etag.as_bytes());
    let Some([own]) = own.as_deref() else {
        return false;
    };
    let mut held = Vec::new();
    for field in fields {
        match entity_tags(field) {
            Some(tags) => held.extend(tags),
            None => return false,
        }
    }
    held.contains(own)
}

/// The opaque tags of an entity-tag list (RFC 9110 8.8.3, `#entity-tag`), without their `W/`
/// prefix and quotes, or `None` when `list` is not one.
fn entity_tags(list: &[u8]) -> Option<Vec<&[u8]>> {
    let etagc = |c: &u8| *c == 0x21 || (0x23..=0x7e).contains(c) || *c >= 0x80;
    let mut tags = Vec::new();
    let mut rest = list;
    loop {
        rest = rest.trim_ascii_start();
        let Some(&first) = rest.first() else {
            return Some(tags);
        };
        if first == b',' {
            rest = &rest[1..];
            continue;
        }
        let quoted = rest.strip_prefix(b"W/").unwrap_or(rest);
        let body = quoted.strip_prefix(b"\"")?;
        let end = body.iter().position(|c| *c == b'"')?;
        let tag = &body[..end];
        if !tag.iter().all(etagc) {
            return None;
        }
        tags.push(tag);
        rest = body[end + 1..].trim_ascii_start();
        match rest.first() {
            None => return Some(tags),
            Some(b',') => rest = &rest[1..],
            Some(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(values: &[&'static str]) -> Vec<HeaderValue> {
        values
            .iter()
            .copied()
            .map(HeaderValue::from_static)
            .collect()
    }

    #[test]
    fn none_match_reads_the_tag_list() {
        let etag = "W/\"abc\"";
        for values in [
            // Weak comparison: the strong and the weak form of the tag both match
            &["\"abc\""][..],
            &["W/\"abc\""],
            &["\"x\", \"abc\""],
            &["\"x\",W/\"abc\""],
            &["\"x\"", "W/\"abc\""],
            // Commas inside a quoted tag, and empty list elements
            &["\"a,b\", \"abc\""],
            &["\"other,*,tag\", W/\"abc\""],
            &[", \"x\" ,, \"abc\" ,"],
            // `*` as the whole value
            &["*"],
            &[" * "],
            &["*", ""],
        ] {
            assert!(none_match(&fields(values), etag), "{values:?}");
        }
        for values in [
            &["\"abcd\""][..],
            &["W/\"x\""],
            &["\"x\"", "\"y\""],
            &[""],
            // A `*` inside a quoted tag is not the wildcard
            &["\"other,*,tag\""],
            &["\"*\""],
            &["\"abc,x\""],
            // `*` beside other tags, in one field or several, is not a valid header
            &["\"x\", *"],
            &["*, \"abc\""],
            &["*", "\"x\""],
            &["*, *"],
            // Malformed lists are ignored, even when they hold the tag
            &["abc"],
            &["\"abc"],
            &["w/\"abc\""],
            &["W/ \"abc\""],
            &["\"a bc\""],
            &["\"x\" \"abc\""],
            &["\"abc\" junk"],
            &["\"abc\"", "junk"],
        ] {
            assert!(!none_match(&fields(values), etag), "{values:?}");
        }
        assert!(!none_match(&fields(&[]), etag));
    }

    #[test]
    fn none_match_compares_a_strong_own_tag_weakly() {
        for held in ["\"abc\"", "W/\"abc\""] {
            assert!(none_match(&fields(&[held]), "\"abc\""), "{held}");
        }
        assert!(!none_match(&fields(&["W/\"abd\""]), "\"abc\""));
    }
}
