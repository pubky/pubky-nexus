use crate::db::connectors::pubky::PubkyConnector;
use crate::models::file::details::FileVariant;
use crate::models::{
    file::{details::FileMeta, FileDetails},
    traits::Collection,
};
use crate::static_processor::{StaticProcessor, StaticStorage};
use crate::types::DynError;
use crate::types::PubkyId;
use axum::body::Bytes;
use log::debug;
use pubky_app_specs::{traits::Validatable, PubkyAppFile};
use tokio::fs::remove_dir_all;

pub async fn put(
    uri: String,
    user_id: PubkyId,
    file_id: String,
    blob: Bytes,
) -> Result<(), DynError> {
    debug!("Indexing new file resource at {}/{}", user_id, file_id);

    // Serialize and validate
    let file_input = <PubkyAppFile as Validatable>::try_from(&blob, &file_id)?;

    debug!("file input {:?}", file_input);

    let file_meta = ingest(&user_id, file_id.as_str(), &file_input).await?;

    // Create FileDetails object
    let file_details =
        FileDetails::from_homeserver(&file_input, uri, user_id.to_string(), file_id, file_meta);

    // save new file into the Graph
    file_details.put_to_graph().await?;

    // Index
    FileDetails::put_to_index(
        &[&[
            file_details.owner_id.clone().as_str(),
            file_details.id.clone().as_str(),
        ]],
        vec![Some(file_details)],
    )
    .await?;

    Ok(())
}

// TODO: Move it into its own process, server, etc
async fn ingest(
    user_id: &PubkyId,
    file_id: &str,
    pubkyapp_file: &PubkyAppFile,
    //client: &PubkyClient,
) -> Result<FileMeta, DynError> {
    let pubky_client = PubkyConnector::get_pubky_client()?;
    let blob = match pubky_client.get(pubkyapp_file.src.as_str()).await? {
        Some(metadata) => metadata,
        None => return Err("Error while fetching file blob".into()),
    };

    let path: String = format!("{}/{}", user_id, file_id);
    let storage_path = StaticStorage::get_storage_path();
    let full_path = format!("{}/{}", storage_path, path);
    StaticStorage::store_blob(FileVariant::Main.to_string(), full_path.to_string(), &blob).await?;

    let urls =
        StaticProcessor::get_file_urls_by_content_type(pubkyapp_file.content_type.as_str(), &path);
    Ok(FileMeta { urls })
}

pub async fn del(user_id: &PubkyId, file_id: String) -> Result<(), DynError> {
    debug!("Deleting File resource at {}/{}", user_id, file_id);
    let result = FileDetails::get_by_ids(
        vec![vec![user_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await?;

    let file = &result[0];

    if let Some(value) = file {
        value.delete().await?;
    }

    let folder_path = format!("{}/{}", user_id, file_id);
    let storage_path = StaticStorage::get_storage_path();
    let full_path = format!("{}/{}", storage_path, folder_path);

    remove_dir_all(full_path).await?;
    Ok(())
}
