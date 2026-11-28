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
    description = "The users `user_id`'s Web of Trust reaches within `depth` follows: the authors a `source=wot` post stream reads, whether or not they posted, never `user_id` itself. The users `user_id` follows come first, then the rest; within each, users with a profile before key-discovery stubs, then those more of `user_id`'s follows follow, then by id, so key-discovery stubs `user_id` does not follow itself rank after every user with a profile. At most 1000, with `truncated` set when the reach holds more. The `ETag` names the set: send it back in `If-None-Match` and an unchanged set is a 304.",
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
    let digest = set.digest();
    let tag = HeaderValue::from_str(&etag(&digest))?;
    if none_match(headers.get_all(IF_NONE_MATCH), &digest) {
        return Ok((StatusCode::NOT_MODIFIED, [(ETAG, tag)]).into_response());
    }
    Ok(([(ETAG, tag)], Json(set)).into_response())
}

/// The `ETag` of a set with [`ReachSet::digest`] `digest`: a weak entity tag, weak because the
/// API's compression layer may encode the body, so the bytes sent are not always the same.
fn etag(digest: &str) -> String {
    format!("W/\"{digest}\"")
}

/// Whether the `If-None-Match` fields name the set with `digest` or are `*` (RFC 9110 13.1.2).
/// The fields are one list (5.3): `*` matches only as the whole value; otherwise each field lists
/// entity tags, quoted strings that may hold commas, each optionally `W/`, compared weakly, so a
/// tag matches when its opaque part is `digest`. A field that is not such a list makes the header
/// invalid, so it is ignored and the set is sent.
fn none_match<'a>(fields: impl IntoIterator<Item = &'a HeaderValue>, digest: &str) -> bool {
    let fields: Vec<&[u8]> = fields
        .into_iter()
        .map(|field| field.as_bytes().trim_ascii())
        .filter(|field| !field.is_empty())
        .collect();
    if fields == [b"*".as_slice()] {
        return true;
    }
    let mut held = Vec::new();
    for field in fields {
        match entity_tags(field) {
            Some(tags) => held.extend(tags),
            None => return false,
        }
    }
    held.contains(&digest.as_bytes())
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
        let digest = "abc";
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
            assert!(none_match(&fields(values), digest), "{values:?}");
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
            assert!(!none_match(&fields(values), digest), "{values:?}");
        }
        assert!(!none_match(&fields(&[]), digest));
    }

    /// The route's own tag names its digest, sent back weak as given or in its strong form.
    #[test]
    fn none_match_takes_the_routes_own_tag_back() {
        let own = etag("abc");
        assert_eq!(own, "W/\"abc\"");
        for held in [own.as_str(), own.trim_start_matches("W/")] {
            let field = vec![HeaderValue::from_str(held).expect("field value")];
            assert!(none_match(&field, "abc"), "{held}");
            assert!(!none_match(&field, "abd"), "{held}");
        }
    }

    /// The `ETag` on the wire is the one the model wrote before the route took the tag over:
    /// a client holding a tag still gets its 304.
    #[test]
    fn etag_of_a_fixed_set_is_unchanged() {
        let set = ReachSet {
            authors: vec![
                "x4rt7xeww7k48jwoomu8gwhsa3t775okm9onhc9dzmwpm8mzupay".to_string(),
                "xzujjk4ubtxcmqcb18itbcgmxf3qyobb7nwi7g88byq3bm1udcqo".to_string(),
            ],
            truncated: false,
        };
        assert_eq!(
            etag(&set.digest()),
            "W/\"fe1af23d1609bbea580a7b9a6b0d5c0cfb8fb3482c6f137abd313b0e6e5fc2d6\""
        );
    }

    #[test]
    fn none_match_takes_obs_text_and_rejects_controls_in_a_tag() {
        fn field(value: &[u8]) -> Vec<HeaderValue> {
            vec![HeaderValue::from_bytes(value).expect("field value")]
        }
        // obs-text is etagc: the list parses, and such a tag matches its own bytes
        assert!(none_match(&field(b"\"\xc3\xa9\", \"abc\""), "abc"));
        assert!(none_match(&field(b"\"\xc3\xa9\""), "\u{e9}"));
        assert!(!none_match(&field(b"\"\xc3\xa9\""), "abc"));
        // HTAB is not etagc, so the header is ignored
        assert!(!none_match(&field(b"\"a\tbc\", \"abc\""), "abc"));
        // DEL can't be in a header value; the list parser rejects it all the same
        assert!(entity_tags(b"\"a\x7fbc\"").is_none());
        assert_eq!(
            entity_tags(b"\"a\x80\", W/\"b\""),
            Some(vec![b"a\x80".as_slice(), b"b".as_slice()])
        );
    }
}
