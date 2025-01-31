use crate::db::graph::exec::OperationOutcome;
use crate::events::error::EventProcessorError;
use crate::models::user::Muted;
use crate::types::DynError;
use log::debug;
use pubky_app_specs::PubkyId;

pub async fn put(user_id: PubkyId, muted_id: PubkyId, _blob: &[u8]) -> Result<(), DynError> {
    debug!("Indexing new mute: {} -> {}", user_id, muted_id);

    // TODO: in case we want to validate the content of this homeserver object or its `created_at` timestamp
    // let _mute = <PubkyAppMute as Validatable>::try_from(&blob, &muted_id).await?;

    sync_put(user_id, muted_id).await
}

pub async fn sync_put(user_id: PubkyId, muted_id: PubkyId) -> Result<(), DynError> {
    // (user_id)-[:MUTED]->(muted_id)
    match Muted::put_to_graph(&user_id, &muted_id).await? {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => {
            let dependency = vec![format!("{muted_id}:user:profile.json")];
            Err(EventProcessorError::MissingDependency { dependency }.into())
        }
        OperationOutcome::CreatedOrDeleted => {
            Muted(vec![muted_id.to_string()])
                .put_to_index(&user_id)
                .await
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
        }
    }
}
