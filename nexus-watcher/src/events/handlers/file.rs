use crate::events::EventProcessorError;

use nexus_common::db::PubkyConnector;
use nexus_common::media::FileVariant;
use nexus_common::media::VariantController;
use nexus_common::models::file::Blob;
use nexus_common::models::{
    file::{FileDetails, FileMeta},
    traits::Collection,
};
use pubky_app_specs::{PubkyAppFile, PubkyAppObject, PubkyId};
use std::path::{Path, PathBuf};
use tokio::fs::remove_dir_all;
use tracing::debug;

pub async fn sync_put(
    file: PubkyAppFile,
    uri: String,
    user_id: PubkyId,
    file_id: String,
    files_path: PathBuf,
) -> Result<(), EventProcessorError> {
    debug!("Indexing new file resource at {}/{}", user_id, file_id);

    let file_meta = ingest(&user_id, file_id.as_str(), &file, files_path).await?;

    // Create FileDetails object
    let file_details =
        FileDetails::from_homeserver(&file, uri, user_id.to_string(), file_id, file_meta);

    // SAVE TO GRAPH
    file_details.put_to_graph().await?;

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
    files_path: PathBuf,
) -> Result<FileMeta, EventProcessorError> {
    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(&pubkyapp_file.src).await?;

    let path = Path::new(&user_id.to_string()).join(file_id);
    let full_path = files_path.join(path.clone());

    let blob = response
        .bytes()
        .await
        .map_err(|e| EventProcessorError::client_error(e.to_string()))?;
    let pubky_app_object = PubkyAppObject::from_uri(&pubkyapp_file.src, &blob)
        .map_err(EventProcessorError::generic)?;

    match pubky_app_object {
        PubkyAppObject::Blob(blob) => {
            Blob::put_to_static(FileVariant::Main.to_string(), full_path, &blob)
                .await
                .map_err(EventProcessorError::static_save_failed)?;

            let urls = VariantController::get_file_urls_by_content_type(
                pubkyapp_file.content_type.as_str(),
                &path,
            );
            Ok(FileMeta { urls })
        }
        _ => Err(EventProcessorError::InvalidEventLine(format!(
            "The file has a source uri that is not a blob path: {}",
            pubkyapp_file.src
        ))),
    }
}

pub async fn del(
    user_id: &PubkyId,
    file_id: String,
    files_path: PathBuf,
) -> Result<(), EventProcessorError> {
    debug!("Deleting File resource at {}/{}", user_id, file_id);
    let result = FileDetails::get_by_ids(&[&[user_id, &file_id]]).await?;

    if !result.is_empty() {
        let file = &result[0];

        if let Some(file_details) = file {
            file_details.delete().await?;
        }

        let folder_path = Path::new(&user_id.to_string()).join(&file_id);
        let full_path = files_path.join(folder_path);

        remove_dir_all(full_path).await?;
    }

    Ok(())
}
