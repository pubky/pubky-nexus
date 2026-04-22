use nexus_common::db::PubkyConnector;
use nexus_common::models::event::{EventProcessorError, EventType};
use nexus_common::models::universal_tags::UniversalTag;
use pubky_app_specs::PubkyAppTag;
use tracing::{debug, instrument};

use super::tag;

/// Second-chance handler for possible universal-tag events.
///
/// Called when `Event::parse_event()` returns `UnrecognizedUri`.
///
/// Returns `None` if the URI isn't an app-specific tag path.
/// Returns `Some(Ok(()))` on success or `Some(Err(...))` on processing failure.
#[instrument(name = "universal_tag", skip_all, fields(uri = %uri))]
pub async fn try_handle(
    event_type: &EventType,
    uri: &str,
) -> Option<Result<(), EventProcessorError>> {
    let tag = UniversalTag::try_from_uri(uri)?;

    debug!(
        name = "universal_tag",
        "Universal tag event: {} {} (app={})", event_type, uri, tag.app
    );

    Some(match event_type {
        EventType::Put => handle_put(&tag, uri).await,
        EventType::Del => handle_del(&tag).await,
    })
}

async fn handle_put(tag: &UniversalTag, uri: &str) -> Result<(), EventProcessorError> {
    // Fetch the tag blob from the homeserver
    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(uri).await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unable to read body>".to_string());
        return Err(EventProcessorError::client_error(format!(
            "Fetch universal tag failed {}: HTTP {status} - {body}",
            uri
        )));
    }

    let blob = response.bytes().await.map_err(|e| {
        EventProcessorError::client_error(format!("Failed to read response body: {e}"))
    })?;

    // Deserialize as PubkyAppTag — if it's not a valid tag, this fails cleanly
    let app_tag: PubkyAppTag = serde_json::from_slice(&blob).map_err(|e| {
        EventProcessorError::generic(format!(
            "Failed to deserialize universal tag at {}: {e}",
            uri
        ))
    })?;

    tag::sync_put_resource(
        app_tag,
        tag.user_id.clone(),
        tag.tag_id.clone(),
        tag.app.clone(),
    )
    .await
}

async fn handle_del(tag: &UniversalTag) -> Result<(), EventProcessorError> {
    tag::del(tag.user_id.clone(), tag.tag_id.clone()).await
}
