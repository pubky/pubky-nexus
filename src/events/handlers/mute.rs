use crate::db::graph::exec::OperationOutcome;
use crate::events::error::EventProcessorError;
use crate::events::retry::event::RetryEvent;
use crate::models::user::Muted;
use crate::types::DynError;
use log::debug;
use pubky_app_specs::{user_uri_builder, PubkyId};

pub async fn sync_put(user_id: PubkyId, muted_id: PubkyId) -> Result<(), DynError> {
    debug!("Indexing new mute: {} -> {}", user_id, muted_id);
    // (user_id)-[:MUTED]->(muted_id)
    match Muted::put_to_graph(&user_id, &muted_id).await? {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => {
            match RetryEvent::generate_index_key(&user_uri_builder(muted_id.to_string())) {
                Some(key) => {
                    let dependency = vec![key];
                    Err(EventProcessorError::MissingDependency { dependency }.into())
                }
                None => Err("Could not generate missing dependency key".into()),
            }
        }
        OperationOutcome::CreatedOrDeleted => {
            Muted(vec![muted_id.to_string()])
                .put_to_index(&user_id)
                .await
                .map_err(|e| EventProcessorError::IndexWriteFailed {
                    message: e.to_string(),
                })?;
            Ok(())
        }
    }
}

pub async fn del(user_id: PubkyId, muted_id: PubkyId) -> Result<(), DynError> {
    debug!("Deleting mute: {} -> {}", user_id, muted_id);
    sync_del(user_id, muted_id).await
}

pub async fn sync_del(user_id: PubkyId, muted_id: PubkyId) -> Result<(), DynError> {
    match Muted::del_from_graph(&user_id, &muted_id).await? {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => Err(EventProcessorError::SkipIndexing.into()),
        OperationOutcome::CreatedOrDeleted => {
            Muted(vec![muted_id.to_string()])
                .del_from_index(&user_id)
                .await
                .map_err(|e| EventProcessorError::IndexWriteFailed {
                    message: e.to_string(),
                })?;
            Ok(())
        }
    }
}
