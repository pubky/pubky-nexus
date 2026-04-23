use pubky_app_specs::{ParsedUri, PubkyId, Resource, PROTOCOL, PUBLIC_PATH};
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
        user_id: PubkyId,
        app: String,
        resource: Resource,
        tag_id: String,
    },
}

impl HomeserverParsedUri {
    /// Returns the user ID from the parsed URI.
    pub fn user_id(&self) -> &PubkyId {
        match self {
            HomeserverParsedUri::AppSpec { user_id, .. } => user_id,
            HomeserverParsedUri::UniversalTag { user_id, .. } => user_id,
        }
    }

    /// Returns the resource from the parsed URI.
    pub fn resource(&self) -> &Resource {
        match self {
            HomeserverParsedUri::AppSpec { resource, .. } => resource,
            HomeserverParsedUri::UniversalTag { resource, .. } => resource,
        }
    }

    /// Returns the app name, if available.
    /// Returns "pubky.app" for AppSpec variants.
    pub fn app(&self) -> &str {
        match self {
            HomeserverParsedUri::AppSpec { .. } => "pubky.app",
            HomeserverParsedUri::UniversalTag { app, .. } => app.as_str(),
        }
    }

    /// Returns the tag ID, if this is a UniversalTag with a tag resource.
    pub fn tag_id(&self) -> Option<&str> {
        match self {
            HomeserverParsedUri::AppSpec { .. } => None,
            HomeserverParsedUri::UniversalTag { tag_id, .. } => Some(tag_id.as_str()),
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
        // Try parsing as a universal tag URI: pubky://<user_id>/pub/<app>/tags/<tag_id>
        // We need to manually parse the URI for non-pubky.app apps.
        let parsed_url = url::Url::parse(uri).map_err(|e| format!("Invalid URL: {}", e))?;

        // Validate the scheme.
        if parsed_url.scheme() != PROTOCOL.trim_end_matches("://") {
            return Err(format!(
                "Invalid URI, must start with '{}': {}",
                PROTOCOL, uri
            ));
        }

        // Extract the user_id from the host.
        let user_id_str = parsed_url
            .host_str()
            .ok_or_else(|| format!("Missing user ID in URI: {}", uri))?;
        let user_id =
            PubkyId::try_from(user_id_str).map_err(|e| format!("Invalid user ID: {e}"))?;

        // Get the path segments.
        let segments: Vec<&str> = parsed_url
            .path_segments()
            .ok_or_else(|| format!("Cannot parse path segments from URI: {}", uri))?
            .collect();

        if segments.len() < 3 {
            return Err(format!("Not enough path segments in URI: {}", uri));
        }

        if segments[0] != PUBLIC_PATH.trim_matches('/') {
            return Err(format!(
                "Expected public path '{}' but got '{}' in URI: {}",
                PUBLIC_PATH, segments[0], uri
            ));
        }

        let app_path = segments[1];

        // Skip pubky.app paths since those are handled by ParsedUri::try_from above.
        if app_path == "pubky.app" {
            return Err(format!(
                "URI is not a recognized pubky-app-specs path or universal tag path: {}",
                uri
            ));
        }

        // Check for /tags/<tag_id> pattern: pubky://<user_id>/pub/<app>/tags/<tag_id>
        let path_after_app = &segments[2..];
        if path_after_app.len() == 2 && path_after_app[0] == "tags" && !path_after_app[1].is_empty()
        {
            let tag_id = path_after_app[1].to_string();

            return Ok(HomeserverParsedUri::UniversalTag {
                user_id,
                app: app_path.to_string(),
                resource: Resource::Tag(tag_id.clone()),
                tag_id,
            });
        }

        // Not a recognized universal tag path
        Err(format!(
            "URI is not a recognized pubky-app-specs path or universal tag path: {}",
            uri
        ))
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
        assert_eq!(parsed.app(), "pubky.app");
    }

    #[test]
    fn test_parse_standard_tag_uri() {
        let user_id = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let tag_id = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";
        let uri = format!("pubky://{user_id}/pub/pubky.app/tags/{tag_id}");
        let parsed = HomeserverParsedUri::try_from(uri.as_str()).expect("Failed to parse tag URI");

        assert!(matches!(parsed, HomeserverParsedUri::AppSpec { .. }));
        assert_eq!(parsed.resource(), &Resource::Tag(tag_id.to_string()));
        assert_eq!(parsed.app(), "pubky.app");
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
        assert_eq!(parsed.app(), "mapky");
        assert_eq!(parsed.resource(), &Resource::Tag(tag_id.to_string()));
        assert_eq!(parsed.tag_id(), Some("ABC123"));
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
        assert_eq!(parsed.app(), "eventky.app");
        assert_eq!(parsed.resource(), &Resource::Tag(tag_id.to_string()));
        assert_eq!(parsed.tag_id(), Some("XYZ789"));
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
    fn test_uppercase_scheme() {
        let user_id = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
        let tag_id = "ABC123";
        let uri = format!("PUBKY://{user_id}/pub/mapky/tags/{tag_id}");
        let result = HomeserverParsedUri::try_from(uri.as_str());
        assert!(result.is_ok());
    }
}
