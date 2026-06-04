use std::path::{Path, PathBuf};

use nexus_common::db::PubkyConnector;
use nexus_common::models::event::{EventProcessorError, EventType};
use nexus_common::models::file::FileDetails;
use nexus_common::models::traits::Collection;
use pubky_app_specs::{PubkyAppFile, PubkyId, APP_PATH, PROTOCOL, PUBLIC_PATH};
use tracing::debug;

use super::file;

/// Info extracted from an app-specific file path:
/// `pubky://<user_id>/pub/<app>/files/<file_id>`.
pub struct AppFileInfo {
    pub user_id: PubkyId,
    pub file_id: String,
    pub uri: String,
}

/// Second-chance handler for app-specific file events that are outside
/// `pubky.app` but still use the universal `PubkyAppFile` schema.
pub async fn try_handle(
    event_type: &EventType,
    uri: &str,
    files_path: &Path,
) -> Option<Result<(), EventProcessorError>> {
    let info = try_parse_app_file_path(uri)?;

    debug!("Universal file event: {} {}", event_type, info.uri);

    Some(match event_type {
        EventType::Put => handle_put(info, files_path.to_path_buf()).await,
        EventType::Del => handle_del(info, files_path.to_path_buf()).await,
    })
}

async fn handle_put(info: AppFileInfo, files_path: PathBuf) -> Result<(), EventProcessorError> {
    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(&info.uri).await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unable to read body>".to_string());
        return Err(EventProcessorError::client_error(format!(
            "Fetch universal file failed {}: HTTP {status} - {body}",
            info.uri
        )));
    }

    let file_json = response
        .bytes()
        .await
        .map_err(|e| EventProcessorError::client_error(e.to_string()))?;

    let app_file: PubkyAppFile = serde_json::from_slice(&file_json).map_err(|e| {
        EventProcessorError::generic(format!(
            "Failed to deserialize universal file at {}: {e}",
            info.uri
        ))
    })?;

    debug!(
        "Ingesting universal file {}/{} (src={})",
        info.user_id, info.file_id, app_file.src
    );

    let pubky = PubkyConnector::get()?;
    let blob_response = pubky.public_storage().get(&app_file.src).await?;
    let raw_bytes = blob_response
        .bytes()
        .await
        .map_err(|e| EventProcessorError::client_error(e.to_string()))?;

    let file_meta = file::ingest_raw(
        &info.user_id,
        &info.file_id,
        &app_file.content_type,
        raw_bytes.to_vec(),
        files_path,
    )
    .await?;

    let file_details = FileDetails::from_homeserver(
        &app_file,
        info.uri,
        info.user_id.to_string(),
        info.file_id,
        file_meta,
    );

    file_details.put_to_graph().await?;

    let owner_id = file_details.owner_id.clone();
    let id = file_details.id.clone();
    FileDetails::put_to_index(
        &[&[owner_id.as_str(), id.as_str()]],
        vec![Some(file_details)],
    )
    .await?;

    Ok(())
}

async fn handle_del(info: AppFileInfo, files_path: PathBuf) -> Result<(), EventProcessorError> {
    file::del(&info.user_id, info.file_id, files_path).await
}

fn try_parse_app_file_path(uri: &str) -> Option<AppFileInfo> {
    let rest = match uri.get(..PROTOCOL.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(PROTOCOL) => &uri[PROTOCOL.len()..],
        _ => return None,
    };

    let slash_pos = rest.find('/')?;
    let user_id_str = &rest[..slash_pos];
    let path = &rest[slash_pos..];

    let path = path.strip_prefix(PUBLIC_PATH)?;
    let (app, file_id) = path.split_once("/files/")?;

    if app == APP_PATH.trim_end_matches('/') {
        return None;
    }

    if app.is_empty() || app.contains('/') || file_id.is_empty() || file_id.contains('/') {
        return None;
    }

    let user_id = match PubkyId::try_from(user_id_str) {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("Invalid user_id '{user_id_str}' in universal file path: {e}");
            return None;
        }
    };

    Some(AppFileInfo {
        user_id,
        file_id: file_id.to_string(),
        uri: uri.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_app_file_path_mapky() {
        let info = try_parse_app_file_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky.app/files/ABC123",
        );
        assert!(info.is_some());
        assert_eq!(info.unwrap().file_id, "ABC123");
    }

    #[test]
    fn test_try_parse_app_file_path_eventky() {
        let info = try_parse_app_file_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/eventky.app/files/XYZ",
        );
        assert!(info.is_some());
        assert_eq!(info.unwrap().file_id, "XYZ");
    }

    #[test]
    fn test_try_parse_app_file_path_pubky_app_returns_none() {
        let info = try_parse_app_file_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/pubky.app/files/123",
        );
        assert!(info.is_none());
    }

    #[test]
    fn test_try_parse_app_file_path_not_file() {
        assert!(try_parse_app_file_path(
            "pubky://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky.app/posts/123"
        )
        .is_none());
    }

    #[test]
    fn test_try_parse_app_file_path_uppercase_scheme() {
        let info = try_parse_app_file_path(
            "PUBKY://8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo/pub/mapky.app/files/ABC",
        );
        assert!(info.is_some());
    }
}
