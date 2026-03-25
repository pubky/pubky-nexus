use std::path::Path;
use std::sync::Arc;

use nexus_common::db::PubkyConnector;
use nexus_common::models::event::EventProcessorError;
use pubky_app_specs::{PubkyAppTag, PubkyId};
use tracing::debug;

use super::tag;
use crate::events::Moderation;

/// Info extracted from a universal tag path: `pubky://<user_id>/pub/<app>/tags/<tag_id>`
pub struct AppTagInfo {
    pub user_id: PubkyId,
    pub app: String,
    pub tag_id: String,
    pub uri: String,
}

/// Second-chance handler for tag events at app-specific paths.
///
/// Called when `Event::parse_event()` fails (returns Err or None) because the URI
/// is at a non-pubky.app path like `/pub/mapky/tags/TAG_ID`.
///
/// Returns `None` if the line isn't an app-specific tag event.
/// Returns `Some(Ok(()))` on success or `Some(Err(...))` on processing failure.
pub async fn try_handle(
    line: &str,
    files_path: &Path,
    _moderation: &Arc<Moderation>,
) -> Option<Result<(), EventProcessorError>> {
    let (event_type, uri) = parse_event_line(line)?;
    let info = try_parse_app_tag_path(uri)?;

    debug!(
        "Universal tag event: {} {} (app={})",
        event_type, info.uri, info.app
    );

    Some(match event_type {
        "PUT" => handle_put(info, files_path).await,
        "DEL" => handle_del(info).await,
        _ => return None,
    })
}

async fn handle_put(info: AppTagInfo, _files_path: &Path) -> Result<(), EventProcessorError> {
    // Fetch the tag blob from the homeserver
    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(&info.uri).await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unable to read body>".to_string());
        return Err(EventProcessorError::client_error(format!(
            "Fetch universal tag failed {}: HTTP {status} - {body}",
            info.uri
        )));
    }

    let blob = response
        .bytes()
        .await
        .map_err(|e| EventProcessorError::client_error(e.to_string()))?;

    // Deserialize as PubkyAppTag — if it's not a valid tag, this fails cleanly
    let app_tag: PubkyAppTag = serde_json::from_slice(&blob).map_err(|e| {
        EventProcessorError::generic(format!(
            "Failed to deserialize universal tag at {}: {e}",
            info.uri
        ))
    })?;

    tag::sync_put_resource(app_tag, info.user_id, info.tag_id, info.app).await
}

async fn handle_del(info: AppTagInfo) -> Result<(), EventProcessorError> {
    // Try app-specific delete first (Resource tags have `app` on TAGGED relationship).
    // If no row found, fall back to app-agnostic delete — this handles the case where
    // sync_put_resource delegated to the standard Post/User flow (InternalKnown),
    // which creates TAGGED relationships WITHOUT `app`.
    let result = tag::del(info.user_id.clone(), info.tag_id.clone(), Some(info.app)).await;
    match result {
        Err(EventProcessorError::SkipIndexing) => {
            // No match with app filter — try without (InternalKnown case)
            tag::del(info.user_id, info.tag_id, None).await
        }
        other => other,
    }
}

/// Parse a raw event line into (event_type, uri).
/// Event lines are formatted as "PUT <uri>" or "DEL <uri>".
fn parse_event_line(line: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    if parts.len() != 2 {
        return None;
    }
    let event_type = parts[0];
    let uri = parts[1];
    if event_type != "PUT" && event_type != "DEL" {
        return None;
    }
    Some((event_type, uri))
}

/// Try to parse a URI as an app-specific tag path.
///
/// Matches: `pubky://<user_id>/pub/<app>/tags/<tag_id>`
/// Returns None if:
/// - Not a pubky:// URI
/// - Not a */tags/* path
/// - App is "pubky.app" (handled by the standard event flow)
fn try_parse_app_tag_path(uri: &str) -> Option<AppTagInfo> {
    // Case-insensitive scheme check per RFC 3986 (safe UTF-8 access)
    let rest = match uri.get(..8) {
        Some(prefix) if prefix.eq_ignore_ascii_case("pubky://") => &uri[8..],
        _ => return None,
    };

    // Split: <user_id>/pub/<app>/tags/<tag_id>
    let slash_pos = rest.find('/')?;
    let user_id_str = &rest[..slash_pos];
    let path = &rest[slash_pos..]; // starts with /

    // Expected: /pub/<app>/tags/<tag_id>
    let path = path.strip_prefix("/pub/")?;

    // Split on /tags/
    let tags_pos = path.find("/tags/")?;
    let app = &path[..tags_pos];
    let tag_id = &path[tags_pos + 6..]; // skip "/tags/"

    // Skip if app is pubky.app — those go through the standard flow
    if app == "pubky.app" {
        return None;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_event_line_put() {
        let (t, u) = parse_event_line("PUT pubky://abc/pub/mapky/tags/123").unwrap();
        assert_eq!(t, "PUT");
        assert_eq!(u, "pubky://abc/pub/mapky/tags/123");
    }

    #[test]
    fn test_parse_event_line_del() {
        let (t, _) = parse_event_line("DEL pubky://abc/pub/mapky/tags/123").unwrap();
        assert_eq!(t, "DEL");
    }

    #[test]
    fn test_parse_event_line_invalid() {
        assert!(parse_event_line("PATCH something").is_none());
        assert!(parse_event_line("malformed").is_none());
    }

    #[test]
    fn test_try_parse_app_tag_path_mapky() {
        let info = try_parse_app_tag_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky/tags/ABC123",
        );
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
}
