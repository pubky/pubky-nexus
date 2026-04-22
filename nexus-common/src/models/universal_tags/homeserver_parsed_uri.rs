use crate::models::universal_tags::UniversalTag;
use pubky_app_specs::{ParsedUri, PubkyId, Resource};
use serde::{Deserialize, Serialize};
use std::convert::{From, TryFrom};

/// Parsed URI representation that can handle both:
/// 1. Standard pubky-app-specs URIs (pubky://<user_id>/pub/pubky.app/...)
/// 2. Universal tag URIs from other apps (pubky://<user_id>/pub/<app>/tags/<tag_id>)
///
/// This is needed because homeservers may return events from applications other than
/// pubky.app (e.g., eventky.app, mapky), and ParsedUri from pubky-app-specs strictly
/// requires the app path to be "pubky.app".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HomeserverParsedUri {
    /// Standard pubky-app-specs ParsedUri (pubky.app path)
    AppSpec {
        user_id: PubkyId,
        resource: Resource,
    },
    /// Universal tag URI from a different app.
    /// Format: pubky://<user_id>/pub/<app>/tags/<tag_id>
    UniversalTag {
        resource: Resource,
        tag: UniversalTag,
    },
}

impl HomeserverParsedUri {
    /// Returns the user ID from the parsed URI.
    pub fn user_id(&self) -> &PubkyId {
        match self {
            HomeserverParsedUri::AppSpec { user_id, .. } => user_id,
            HomeserverParsedUri::UniversalTag { tag, .. } => &tag.user_id,
        }
    }

    /// Returns the resource from the parsed URI.
    pub fn resource(&self) -> &Resource {
        match self {
            HomeserverParsedUri::AppSpec { resource, .. } => resource,
            HomeserverParsedUri::UniversalTag { resource, .. } => resource,
        }
    }
}

impl From<ParsedUri> for HomeserverParsedUri {
    fn from(parsed: ParsedUri) -> Self {
        // ParsedUri from pubky-app-specs is always a pubky.app path
        HomeserverParsedUri::AppSpec {
            user_id: parsed.user_id,
            resource: parsed.resource,
        }
    }
}

impl TryFrom<&str> for HomeserverParsedUri {
    type Error = String;

    fn try_from(uri: &str) -> Result<Self, Self::Error> {
        // First, try parsing as a standard pubky-app-specs ParsedUri (pubky.app path).
        // This handles URL validation, scheme checking, user_id extraction, and resource parsing
        // for pubky.app URIs in one call.
        if let Ok(parsed_uri) = ParsedUri::try_from(uri) {
            return Ok(HomeserverParsedUri::AppSpec {
                user_id: parsed_uri.user_id,
                resource: parsed_uri.resource,
            });
        }

        // If ParsedUri::try_from failed, the URI might be from a different app.
        // Delegate to UniversalTag::try_from_uri which handles universal tag parsing
        // (pubky://<user_id>/pub/<app>/tags/<tag_id>) including case-insensitive scheme
        // validation and path segment validation.
        let universal_tag = UniversalTag::try_from_uri(uri).ok_or_else(|| {
            format!(
                "URI is not a recognized pubky-app-specs path or universal tag path: {}",
                uri
            )
        })?;

        Ok(HomeserverParsedUri::UniversalTag {
            resource: Resource::Tag(universal_tag.tag_id.clone()),
            tag: universal_tag,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_post_uri() {
        let user_id = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let post_id = "0032SSN7Q4EVG";
        let uri = format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}");
        let parsed = HomeserverParsedUri::try_from(uri.as_str()).expect("Failed to parse post URI");

        assert!(matches!(parsed, HomeserverParsedUri::AppSpec { .. }));
        assert_eq!(parsed.resource(), &Resource::Post(post_id.to_string()));
    }

    #[test]
    fn test_parse_standard_tag_uri() {
        let user_id = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let tag_id = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";
        let uri = format!("pubky://{user_id}/pub/pubky.app/tags/{tag_id}");
        let parsed = HomeserverParsedUri::try_from(uri.as_str()).expect("Failed to parse tag URI");

        assert!(matches!(parsed, HomeserverParsedUri::AppSpec { .. }));
        assert_eq!(parsed.resource(), &Resource::Tag(tag_id.to_string()));
    }

    #[test]
    fn test_parse_universal_tag_uri_mapky() {
        let user_id = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let tag_id = "ABC123";
        let uri = format!("pubky://{user_id}/pub/mapky/tags/{tag_id}");
        let parsed =
            HomeserverParsedUri::try_from(uri.as_str()).expect("Failed to parse mapky tag URI");

        assert!(matches!(parsed, HomeserverParsedUri::UniversalTag { .. }));
        assert_eq!(parsed.user_id(), &PubkyId::try_from(user_id).unwrap());
        assert_eq!(parsed.resource(), &Resource::Tag(tag_id.to_string()));

        // Access the UniversalTag struct directly
        if let HomeserverParsedUri::UniversalTag { tag, .. } = parsed {
            assert_eq!(tag.app, "mapky");
            assert_eq!(tag.tag_id, "ABC123");
        }
    }

    #[test]
    fn test_parse_universal_tag_uri_eventky() {
        let user_id = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let tag_id = "XYZ789";
        let uri = format!("pubky://{user_id}/pub/eventky.app/tags/{tag_id}");
        let parsed =
            HomeserverParsedUri::try_from(uri.as_str()).expect("Failed to parse eventky tag URI");

        assert!(matches!(parsed, HomeserverParsedUri::UniversalTag { .. }));
        assert_eq!(parsed.user_id(), &PubkyId::try_from(user_id).unwrap());
        assert_eq!(parsed.resource(), &Resource::Tag(tag_id.to_string()));

        if let HomeserverParsedUri::UniversalTag { tag, .. } = parsed {
            assert_eq!(tag.app, "eventky.app");
            assert_eq!(tag.tag_id, "XYZ789");
        }
    }

    #[test]
    fn test_reject_universal_non_tag_path() {
        let user_id = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let uri = format!("pubky://{user_id}/pub/eventky.app/posts/123");
        let result = HomeserverParsedUri::try_from(uri.as_str());
        assert!(result.is_err(), "Should reject non-tag universal paths");
    }

    #[test]
    fn test_reject_non_pubky_scheme() {
        let result = HomeserverParsedUri::try_from("https://example.com/pub/pubky.app/");
        assert!(result.is_err(), "Should reject non-pubky scheme");
    }

    #[test]
    fn test_reject_missing_user_id() {
        let result = HomeserverParsedUri::try_from("pubky:///pub/pubky.app/");
        assert!(result.is_err(), "Should reject missing user ID");
    }

    #[test]
    fn test_parse_universal_tag_uri_case_insensitive_scheme() {
        let user_id = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let tag_id = "ABC123";
        let uri = format!("PUBKY://{user_id}/pub/mapky/tags/{tag_id}");
        let parsed = HomeserverParsedUri::try_from(uri.as_str())
            .expect("Failed to parse case-insensitive URI");

        assert!(matches!(parsed, HomeserverParsedUri::UniversalTag { .. }));
    }
}