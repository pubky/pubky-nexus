use crate::models::user::Muted;
use crate::types::DynError;
use crate::types::PubkyId;
use axum::body::Bytes;
use log::debug;

pub async fn put(user_id: PubkyId, muted_id: PubkyId, _blob: Bytes) -> Result<(), DynError> {
    debug!("Indexing new mute: {} -> {}", user_id, muted_id);

    // TODO: in case we want to validate the content of this homeserver object or its `created_at` timestamp
    // let _mute = <PubkyAppMute as Validatable>::try_from(&blob, &muted_id).await?;

    sync_put(user_id, muted_id).await
}

pub async fn sync_put(user_id: PubkyId, muted_id: PubkyId) -> Result<(), DynError> {
    // SAVE TO GRAPH
    // (user_id)-[:MUTED]->(muted_id)
    Muted::put_to_graph(&user_id, &muted_id).await?;

    // SAVE TO INDEX
    Muted(vec![muted_id.to_string()])
        .put_to_index(&user_id)
        .await?;

    Ok(())
}

pub async fn del(user_id: PubkyId, muted_id: PubkyId) -> Result<(), DynError> {
    debug!("Deleting mute: {} -> {}", user_id, muted_id);
    sync_del(user_id, muted_id).await
}

pub async fn sync_del(user_id: PubkyId, muted_id: PubkyId) -> Result<(), DynError> {
    // DELETE FROM GRAPH
    Muted::del_from_graph(&user_id, &muted_id).await?;

    // REMOVE FROM INDEX
    Muted(vec![muted_id.to_string()])
        .del_from_index(&user_id)
        .await?;

    Ok(())
}
