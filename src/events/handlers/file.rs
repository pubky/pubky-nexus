use crate::db::connectors::pubky::PubkyConnector;
use crate::events::error::EventProcessorError;
use crate::models::{
    file::{
        details::{FileMeta, FileVariant},
        FileDetails,
    },
    traits::Collection,
};
use crate::static_processor::{StaticProcessor, StaticStorage};
use crate::types::DynError;
use log::{debug, error};
use pubky_app_specs::{PubkyAppFile, PubkyAppObject, PubkyId};
use tokio::fs::remove_dir_all;

pub async fn sync_put(
    file: PubkyAppFile,
    uri: String,
    user_id: PubkyId,
    file_id: String,
) -> Result<(), DynError> {
    debug!("Indexing new file resource at {}/{}", user_id, file_id);
    debug!("file input {:?}", file);

    // Ingest the file and store its blob content
    let file_meta = ingest(&user_id, file_id.as_str(), &file).await?;

    // Build the FileDetails (this may combine info from the homeserver and the meta)
    let file_details =
        FileDetails::from_homeserver(&file, uri, user_id.to_string(), file_id, file_meta);

    // Save into the graph and index
    file_details.put_to_graph().await?;
    FileDetails::put_to_index(
        &[&[file_details.owner_id.as_str(), file_details.id.as_str()]],
        vec![Some(file_details.clone())],
    )
    .await?;

    Ok(())
}

async fn ingest(
    user_id: &PubkyId,
    file_id: &str,
    pubkyapp_file: &PubkyAppFile,
) -> Result<FileMeta, DynError> {
    // Retrieve the file blob via PubkyConnector
    let pubky_client = PubkyConnector::get_pubky_client()?;
    let response = match pubky_client.get(&pubkyapp_file.src).send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("EVENT ERROR: could not retrieve file src blob");
            return Err(e.into());
        }
    };

    // Read the response bytes and validate the file using PubkyAppObject importer
    let blob_bytes = response.bytes().await?;
    let pubky_app_object = PubkyAppObject::from_uri(&pubkyapp_file.src, &blob_bytes)?;
    match pubky_app_object {
        PubkyAppObject::Blob(valid_blob) => {
            // Build a relative path based on user and file IDs
            let relative_path = format!("{}/{}", user_id, file_id);
            let storage_path = StaticStorage::get_storage_path();
            let full_path = format!("{}/{}", storage_path, relative_path);

            // Store the blob via the static storage abstraction
            StaticStorage::store_blob(FileVariant::Main.to_string(), full_path, &valid_blob.0)
                .await?;

            // Process the stored file and get URLs based on its content type
            let urls = StaticProcessor::get_file_urls_by_content_type(
                pubkyapp_file.content_type.as_str(),
                &relative_path,
            );
            Ok(FileMeta { urls })
        }
        _ => Err(EventProcessorError::InvalidEventLine {
            message: format!(
                "The file has a source uri that is not a blob path: {}",
                pubkyapp_file.src
            ),
        }
        .into()),
    }
}

pub async fn del(user_id: &PubkyId, file_id: String) -> Result<(), DynError> {
    debug!("Deleting File resource at {}/{}", user_id, file_id);
    let result = FileDetails::get_by_ids(
        vec![vec![user_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await?;

    if !result.is_empty() {
        if let Some(file) = &result[0] {
            file.delete().await?;
        }

        let folder_path = format!("{}/{}", user_id, file_id);
        let storage_path = StaticStorage::get_storage_path();
        let full_path = format!("{}/{}", storage_path, folder_path);

        remove_dir_all(full_path).await?;
    }

    Ok(())
}
