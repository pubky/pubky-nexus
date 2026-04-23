use nexus_common::db::PubkyConnector;
use nexus_common::models::event::{EventProcessorError, EventType};
use nexus_common::universal_tag::app_tag_info::{try_parse_app_tag_path, AppTagInfo};
use tracing::{debug, instrument};

use super::tag;

/// Second-chance handler for possible universal-tag events.
///
/// Called when `Event::parse_event()` returns `UnrecognizedUri`.
///
/// Returns `None` if the URI isn't an app-specific tag path.
/// Returns `Some(Ok(()))` on success or `Some(Err(...))` on processing failure.
#[instrument(name="universal_tag", skip(event_type, uri), fields(uri = %uri))]
pub async fn try_handle(
    event_type: &EventType,
    uri: &str,
) -> Option<Result<(), EventProcessorError>> {
    let info = try_parse_app_tag_path(uri)?;
    tracing::Span::current().record("app_type", info.app.as_str());

    debug!(
        "Universal tag event: {} {} (app={})",
        event_type, info.uri, info.app
    );

    Some(match event_type {
        EventType::Put => handle_put(info).await,
        EventType::Del => handle_del(info).await,
    })
}

async fn handle_put(info: AppTagInfo) -> Result<(), EventProcessorError> {
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

    let blob = response.bytes().await.map_err(|e| {
        EventProcessorError::client_error(format!("Failed to read response body: {e}"))
    })?;

    // Deserialize as PubkyAppTag — if it's not a valid tag, this fails cleanly
    let app_tag: pubky_app_specs::PubkyAppTag = serde_json::from_slice(&blob).map_err(|e| {
        EventProcessorError::generic(format!(
            "Failed to deserialize universal tag at {}: {e}",
            info.uri
        ))
    })?;

    tag::sync_put_resource(app_tag, info.user_id, info.tag_id, info.app).await
}

async fn handle_del(info: AppTagInfo) -> Result<(), EventProcessorError> {
    tag::del(info.user_id, info.tag_id).await
}
