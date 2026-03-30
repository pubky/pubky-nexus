pub mod stream;
pub mod tag;
pub mod view;

use pubky_app_specs::{ParsedUri, Resource};
use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// URI Normalization (universal_tags_specs.md Section 5)
// ---------------------------------------------------------------------------

/// Normalizes a URI for deterministic Resource identification.
///
/// Rules:
/// - Lowercase scheme and host
/// - Strip default ports (80 for http, 443 for https)
/// - Strip fragments and userinfo
/// - Preserve path and query as-is
/// - Fallback for non-standard schemes that fail URL parsing
pub fn normalize_uri(uri: &str) -> Result<String, String> {
    match Url::parse(uri) {
        Ok(parsed) => Ok(normalize_parsed_url(&parsed)),
        Err(_) => {
            // Fallback for non-standard schemes (e.g., nostr:note1abc...)
            // that may not parse as URLs (no // authority)
            if let Some(colon_pos) = uri.find(':') {
                let scheme = &uri[..colon_pos];
                let remainder = &uri[colon_pos + 1..];
                Ok(format!("{}:{}", scheme.to_ascii_lowercase(), remainder))
            } else {
                Err(format!("Invalid URI: {uri}"))
            }
        }
    }
}

fn normalize_parsed_url(parsed: &Url) -> String {
    let scheme = parsed.scheme(); // already lowercase per url crate

    // For non-hierarchical schemes (no authority, e.g. nostr:note1abc),
    // the url crate parses them as "cannot-be-a-base" URLs.
    // Return scheme + ":" + opaque path (no //)
    if parsed.cannot_be_a_base() {
        // Strip fragment from the opaque form
        let full = parsed.as_str();
        let without_fragment = match full.find('#') {
            Some(pos) => &full[..pos],
            None => full,
        };
        return without_fragment.to_string();
    }

    let host = parsed
        .host_str()
        .map(|h| h.to_ascii_lowercase())
        .unwrap_or_default();

    // Strip default ports
    let port = match (parsed.port(), scheme) {
        (Some(80), "http") => None,
        (Some(443), "https") => None,
        (other, _) => other,
    };

    let path = parsed.path();
    let query = parsed.query();
    // Fragment and userinfo are discarded

    let mut result = format!("{scheme}://{host}");
    if let Some(p) = port {
        result.push_str(&format!(":{p}"));
    }
    result.push_str(path);
    if let Some(q) = query {
        result.push('?');
        result.push_str(q);
    }
    result
}

// ---------------------------------------------------------------------------
// Resource ID (universal_tags_specs.md Section 6)
// ---------------------------------------------------------------------------

/// Generates a deterministic 32-char hex Resource ID from a normalized URI.
///
/// `resource_id = hex(BLAKE3(normalized_uri)[0..16])`
pub fn resource_id(normalized_uri: &str) -> String {
    let hash = blake3::hash(normalized_uri.as_bytes());
    hex::encode(&hash.as_bytes()[..16])
}

// ---------------------------------------------------------------------------
// URI Classification (universal_tags_specs.md Section 7)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum UriCategory {
    /// pubky:// URI matching a known schema (posts, users)
    InternalKnown,
    /// pubky:// URI NOT matching any known schema (e.g., eventky events)
    InternalUnknown,
    /// Non-pubky:// URI (https://, nostr:, ipfs://, etc.)
    External,
}

/// Classifies a tag's target URI into one of three categories.
/// Scheme check is case-insensitive per RFC 3986.
pub fn classify_uri(uri: &str) -> UriCategory {
    let is_pubky = uri
        .get(..8)
        .is_some_and(|s| s.eq_ignore_ascii_case("pubky://"));
    if is_pubky {
        match ParsedUri::try_from(uri) {
            Ok(parsed) if matches!(parsed.resource, Resource::Post(_) | Resource::User) => {
                UriCategory::InternalKnown
            }
            _ => UriCategory::InternalUnknown,
        }
    } else {
        UriCategory::External
    }
}

/// Extracts the scheme from a URI string.
pub fn extract_scheme(uri: &str) -> String {
    match Url::parse(uri) {
        Ok(parsed) => parsed.scheme().to_string(),
        Err(_) => {
            // Fallback: extract up to first ':'
            uri.split(':')
                .next()
                .unwrap_or("unknown")
                .to_ascii_lowercase()
        }
    }
}

// ---------------------------------------------------------------------------
// ResourceDetails
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ResourceDetails {
    pub id: String,
    pub uri: String,
    pub scheme: String,
    pub indexed_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- normalize_uri test vectors from spec Section 5 --

    #[test]
    fn test_normalize_https_with_default_port_and_fragment() {
        let result = normalize_uri("HTTPS://Example.COM:443/path?q=1#frag").unwrap();
        assert_eq!(result, "https://example.com/path?q=1");
    }

    #[test]
    fn test_normalize_root_trailing_slash() {
        let result = normalize_uri("https://example.com").unwrap();
        assert_eq!(result, "https://example.com/");
    }

    #[test]
    fn test_normalize_non_default_port() {
        let result = normalize_uri("http://example.com:8080/path").unwrap();
        assert_eq!(result, "http://example.com:8080/path");
    }

    #[test]
    fn test_normalize_pubky_uri() {
        let input = "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/eventky.app/events/E001";
        let result = normalize_uri(input).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_normalize_query_order_preserved() {
        let result = normalize_uri("HTTPS://Example.COM/path?b=2&a=1").unwrap();
        assert_eq!(result, "https://example.com/path?b=2&a=1");
    }

    #[test]
    fn test_normalize_strip_userinfo() {
        let result = normalize_uri("https://user:pass@example.com/path").unwrap();
        assert_eq!(result, "https://example.com/path");
    }

    #[test]
    fn test_normalize_nostr_fallback() {
        let result = normalize_uri("nostr:note1abc123...").unwrap();
        assert_eq!(result, "nostr:note1abc123...");
    }

    #[test]
    fn test_normalize_http_default_port() {
        let result = normalize_uri("HTTP://Example.COM:80/").unwrap();
        assert_eq!(result, "http://example.com/");
    }

    // -- resource_id tests --

    #[test]
    fn test_resource_id_deterministic() {
        let uri = "https://example.com/path?q=1";
        let id1 = resource_id(uri);
        let id2 = resource_id(uri);
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 32);
    }

    #[test]
    fn test_resource_id_different_uris() {
        let id1 = resource_id("https://example.com/a");
        let id2 = resource_id("https://example.com/b");
        assert_ne!(id1, id2);
    }

    // -- classify_uri tests --

    #[test]
    fn test_classify_external_https() {
        assert_eq!(
            classify_uri("https://example.com/article"),
            UriCategory::External
        );
    }

    #[test]
    fn test_classify_external_nostr() {
        assert_eq!(classify_uri("nostr:note1abc123"), UriCategory::External);
    }

    #[test]
    fn test_classify_internal_unknown_eventky() {
        // eventky URI is pubky:// but not a recognized pubky.app resource
        assert_eq!(
            classify_uri("pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/eventky.app/events/E001"),
            UriCategory::InternalUnknown
        );
    }

    #[test]
    fn test_classify_uppercase_pubky_scheme() {
        // RFC 3986: schemes are case-insensitive
        assert_eq!(
            classify_uri("PUBKY://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/eventky.app/events/E001"),
            UriCategory::InternalUnknown
        );
    }

    // -- extract_scheme tests --

    #[test]
    fn test_extract_scheme_https() {
        assert_eq!(extract_scheme("https://example.com"), "https");
    }

    #[test]
    fn test_extract_scheme_pubky() {
        assert_eq!(extract_scheme("pubky://somekey/pub/app/tags/123"), "pubky");
    }

    #[test]
    fn test_extract_scheme_nostr() {
        assert_eq!(extract_scheme("nostr:note1abc"), "nostr");
    }
}
