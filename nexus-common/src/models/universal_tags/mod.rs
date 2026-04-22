mod homeserver_parsed_uri;

pub use homeserver_parsed_uri::HomeserverParsedUri;

use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};

/// Parsed universal tag from a URI: `pubky://<user_id>/pub/<app>/tags/<tag_id>`
///
/// This struct encapsulates the data extracted from app-specific universal tag URIs
/// that are not handled by the standard pubky.app ParsedUri (which requires the app
/// path to be "pubky.app").
///
/// Example URIs:
/// - `pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC123`
/// - `pubky://<user_id>/pub/eventky.app/tags/EVT001`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UniversalTag {
    /// The PubkyId of the user who owns this tag resource
    pub user_id: PubkyId,
    /// The app name (e.g., "mapky", "eventky.app")
    pub app: String,
    /// The unique tag identifier
    pub tag_id: String,
}

impl UniversalTag {
    /// Try to parse a URI as a universal tag path.
    ///
    /// Returns `Some(UniversalTag)` if the URI matches the pattern
    /// `pubky://<user_id>/pub/<app>/tags/<tag_id>`.
    ///
    /// Returns `None` if:
    /// - The URI is not a pubky:// URI
    /// - The app is "pubky.app" (handled by the standard event flow)
    /// - The path does not contain `/tags/<tag_id>`
    /// - The app or tag_id contains slashes (invalid segments)
    /// - The user_id or tag_id is empty
    pub fn try_from_uri(uri: &str) -> Option<Self> {
        // Case-insensitive scheme check per RFC 3986 (safe UTF-8 access)
        let rest = to_ascii_lower_prefix(uri, "pubky://")?;

        // Split: <user_id>/pub/<app>/tags/<tag_id>
        let (user_id_str, rest) = rest.split_once('/')?;

        // Validate user_id is not empty
        if user_id_str.is_empty() {
            return None;
        }

        let user_id = PubkyId::try_from(user_id_str).ok()?;

        let rest = rest.strip_prefix("pub/")?;

        // Split on /tags/
        let (app, tag_id) = rest.split_once("/tags/")?;

        // Skip if app is pubky.app — those go through the standard flow
        if app == "pubky.app" {
            return None;
        }

        // Validate app and tag_id are not empty and don't contain slashes
        if app.is_empty() || tag_id.is_empty() || app.contains('/') || tag_id.contains('/') {
            return None;
        }

        Some(Self {
            user_id,
            app: app.to_string(),
            tag_id: tag_id.to_string(),
        })
    }
}

/// Strip a case-insensitive prefix from a string.
fn to_ascii_lower_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() < prefix.len() {
        return None;
    }
    if s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE_URI: &str =
        "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC123";

    #[test]
    fn test_try_from_uri_mapky() {
        let tag = UniversalTag::try_from_uri(BASE_URI);
        assert!(tag.is_some());
        let tag = tag.unwrap();
        assert_eq!(
            tag.user_id,
            PubkyId::try_from("8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo").unwrap()
        );
        assert_eq!(tag.app, "mapky");
        assert_eq!(tag.tag_id, "ABC123");
    }

    #[test]
    fn test_try_from_uri_eventky() {
        let uri = "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/eventky.app/tags/EVT001";
        let tag = UniversalTag::try_from_uri(uri).unwrap();
        assert_eq!(tag.app, "eventky.app");
        assert_eq!(tag.tag_id, "EVT001");
    }

    #[test]
    fn test_try_from_uri_rejects_pubky_app() {
        // pubky.app paths should be handled by the standard ParsedUri
        let uri =
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/pubky.app/tags/TAG1";
        assert!(UniversalTag::try_from_uri(uri).is_none());
    }

    #[test]
    fn test_try_from_uri_non_pubky_scheme() {
        assert!(UniversalTag::try_from_uri("https://example.com/pub/mapky/tags/ABC").is_none());
    }

    #[test]
    fn test_try_from_uri_not_a_tag_path() {
        let uri =
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/posts/123";
        assert!(UniversalTag::try_from_uri(uri).is_none());
    }

    #[test]
    fn test_try_from_uri_mixed_case_scheme() {
        let uri =
            "PUBKY://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC123";
        let tag = UniversalTag::try_from_uri(uri);
        assert!(tag.is_some(), "Should handle mixed-case PUBKY:// scheme");
    }

    #[test]
    fn test_try_from_uri_mixed_case_pubky() {
        let uri =
            "Pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC123";
        let tag = UniversalTag::try_from_uri(uri);
        assert!(tag.is_some(), "Should handle mixed-case Pubky:// scheme");
    }

    #[test]
    fn test_try_from_uri_slash_in_app_returns_none() {
        assert!(UniversalTag::try_from_uri(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/my/app/tags/ABC"
        )
        .is_none());
    }

    #[test]
    fn test_try_from_uri_slash_in_tag_returns_none() {
        assert!(UniversalTag::try_from_uri(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC/DEF"
        )
        .is_none());
    }

    #[test]
    fn test_try_from_uri_empty_app_returns_none() {
        assert!(UniversalTag::try_from_uri(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub//tags/ABC"
        )
        .is_none());
    }

    #[test]
    fn test_try_from_uri_empty_tag_returns_none() {
        assert!(UniversalTag::try_from_uri(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/"
        )
        .is_none());
    }
}
