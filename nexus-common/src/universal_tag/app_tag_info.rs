use pubky_app_specs::PubkyId;

/// Info extracted from a universal tag path: `pubky://<user_id>/pub/<app>/tags/<tag_id>`
pub struct AppTagInfo {
    pub user_id: PubkyId,
    pub app: String,
    pub tag_id: String,
    pub uri: String,
}

/// Try to parse a URI as an app-specific tag path.
///
/// Matches: `pubky://<user_id>/pub/<app>/tags/<tag_id>`
/// Returns None if:
/// - Not a pubky:// URI
/// - Not a */tags/* path
/// - App is "pubky.app" (handled by the standard event flow)
/// - App or tag_id contains slashes (invalid segments)
pub fn try_parse_app_tag_path(uri: &str) -> Option<AppTagInfo> {
    // Case-insensitive scheme check per RFC 3986 (safe UTF-8 access)
    let rest = to_ascii_lower_prefix(uri, "pubky://")?;

    // Split: <user_id>/pub/<app>/tags/<tag_id>
    let (user_id_str, rest) = rest.split_once('/')?;
    let rest = rest.strip_prefix("pub/")?;

    // Split on /tags/
    let (app, tag_id) = rest.split_once("/tags/")?;

    // Skip if app is pubky.app — those go through the standard flow
    if app == "pubky.app" {
        return None;
    }

    // Strip query string (?...) or fragment (#...) from tag_id
    let tag_id = tag_id
        .find(|c| c == '?' || c == '#')
        .map_or(tag_id, |pos| &tag_id[..pos]);

    // Validate: app must be a single path segment, tag_id must not contain slashes
    if app.is_empty() || app.contains('/') || tag_id.is_empty() || tag_id.contains('/') {
        return None;
    }

    let user_id = match PubkyId::try_from(user_id_str) {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("Invalid user_id '{user_id_str}' in universal tag path: {e}");
            return None;
        }
    };

    Some(AppTagInfo {
        user_id,
        app: app.to_string(),
        tag_id: tag_id.to_string(),
        uri: uri.to_string(),
    })
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
    fn test_try_parse_app_tag_path_mapky() {
        let info = try_parse_app_tag_path(BASE_URI);
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.app, "mapky");
        assert_eq!(info.tag_id, "ABC123");
    }

    #[test]
    fn test_try_parse_app_tag_path_eventky() {
        let info = try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/eventky.app/tags/XYZ",
        );
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.app, "eventky.app");
        assert_eq!(info.tag_id, "XYZ");
    }

    #[test]
    fn test_try_parse_app_tag_path_pubky_app_returns_none() {
        let info = try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/pubky.app/tags/123",
        );
        assert!(info.is_none());
    }

    #[test]
    fn test_try_parse_app_tag_path_not_pubky() {
        assert!(try_parse_app_tag_path("https://example.com/tags/123").is_none());
    }

    #[test]
    fn test_try_parse_app_tag_path_no_tags_segment() {
        assert!(try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/events/123"
        )
        .is_none());
    }

    #[test]
    fn test_try_parse_app_tag_path_uppercase_scheme() {
        let info = try_parse_app_tag_path(
            "PUBKY://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC123",
        );
        assert!(info.is_some(), "Should handle uppercase PUBKY:// scheme");
        assert_eq!(info.unwrap().app, "mapky");
    }

    #[test]
    fn test_try_parse_app_tag_path_mixed_case_scheme() {
        let info = try_parse_app_tag_path(
            "Pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/XYZ",
        );
        assert!(info.is_some(), "Should handle mixed-case Pubky:// scheme");
    }

    #[test]
    fn test_try_parse_app_tag_path_slash_in_app_returns_none() {
        assert!(try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/my/app/tags/ABC"
        )
        .is_none());
    }

    #[test]
    fn test_try_parse_app_tag_path_slash_in_tag_returns_none() {
        assert!(try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC/DEF"
        )
        .is_none());
    }

    #[test]
    fn test_try_parse_app_tag_path_empty_app_returns_none() {
        assert!(try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub//tags/ABC"
        )
        .is_none());
    }

    #[test]
    fn test_try_parse_app_tag_path_empty_tag_returns_none() {
        assert!(try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/"
        )
        .is_none());
    }

    #[test]
    fn test_try_parse_app_tag_path_query_string_stripped() {
        let info = try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC123?foo=bar",
        );
        assert!(info.is_some(), "Should accept URI with query string");
        assert_eq!(
            info.unwrap().tag_id,
            "ABC123",
            "tag_id must not include query string"
        );
    }

    #[test]
    fn test_try_parse_app_tag_path_fragment_stripped() {
        let info = try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC123#section",
        );
        assert!(info.is_some(), "Should accept URI with fragment");
        assert_eq!(
            info.unwrap().tag_id,
            "ABC123",
            "tag_id must not include fragment"
        );
    }

    #[test]
    fn test_try_parse_app_tag_path_empty_tag_after_query_returns_none() {
        // tag_id becomes empty after stripping the query string
        assert!(try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/?foo=bar"
        )
        .is_none());
    }
}
