//! Namespace post versions publish a self reference; the envelope was validated in translation.
use crate::errors::EventProcessorError;
use nexus_common::models::app_post::AppPostUri;
use nexus_common::models::event::resource_changes::{ResourceChange, ResourceChangeOp};

pub async fn put(path: &AppPostUri) -> Result<(), EventProcessorError> {
    ResourceChange::post(path, ResourceChangeOp::Put)?
        .store()
        .await?;
    Ok(())
}

pub async fn del(path: &AppPostUri) -> Result<(), EventProcessorError> {
    ResourceChange::post(path, ResourceChangeOp::Del)?
        .store()
        .await?;
    Ok(())
}
