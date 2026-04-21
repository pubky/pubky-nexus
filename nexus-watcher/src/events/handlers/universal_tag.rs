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
        EventType::Del => handle_del(&tag, uri).await,
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

/// Handle deletion of an app-specific tag.
///
/// This function is called both from [`try_handle`] and directly from the event
/// processing flow in [`crate::events`] for universal-tag DEL events.
#[instrument(name = "universal_tag.del", skip(tag, uri), fields(uri = %uri, app = %tag.app))]
pub async fn handle_del(tag: &UniversalTag, uri: &str) -> Result<(), EventProcessorError> {
    // Try app-specific delete first (Resource tags have `app` on TAGGED relationship).
    // If no row found, fall back to app-agnostic delete — this handles the case where
    // sync_put_resource delegated to the standard Post/User flow (InternalKnown),
    // which creates TAGGED relationships WITHOUT `app`.
    let result = tag::del(
        tag.user_id.clone(),
        tag.tag_id.clone(),
        Some(tag.app.clone()),
    )
    .await;
    match result {
        Err(EventProcessorError::SkipIndexing) => {
            // No match with app filter — try without (InternalKnown case)
            tag::del(tag.user_id.clone(), tag.tag_id.clone(), None).await
        }
        other => other,
    }
}
