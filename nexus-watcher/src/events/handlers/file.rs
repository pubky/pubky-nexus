use crate::events::errors::EventProcessorError;
use crate::handle_indexing_results;
use nexus_common::db::DbError;
use nexus_common::db::PubkyClient;
use nexus_common::media::FileVariant;
use nexus_common::media::VariantController;
use nexus_common::models::file::Blob;
use nexus_common::models::{
    file::{FileDetails, FileMeta},
    traits::Collection,
};
use nexus_common::types::DynError;
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
) -> Result<(), DynError> {
    debug!("Indexing new file resource at {}/{}", user_id, file_id);

    let file_meta = ingest(&user_id, file_id.as_str(), &file, files_path).await?;

    // Create FileDetails object
    let file_details =
        FileDetails::from_homeserver(&file, uri, user_id.to_string(), file_id, file_meta);

    // SAVE TO GRAPH
    file_details
        .put_to_graph()
        .await
        .map_err(|e| EventProcessorError::GraphQueryFailed {
            message: format!("{e:?}"),
        })?;

    // SAVE TO INDEX
    let indexing_result = FileDetails::put_to_index(
        &[&[
            file_details.owner_id.clone().as_str(),
            file_details.id.clone().as_str(),
        ]],
        vec![Some(file_details)],
    )
    .await;

    handle_indexing_results!(indexing_result);

    Ok(())
}

// TODO: Move it into its own process, server, etc
async fn ingest(
    user_id: &PubkyId,
    file_id: &str,
    pubkyapp_file: &PubkyAppFile,
    files_path: PathBuf,
) -> Result<FileMeta, DynError> {
    let response;
    {
        let pubky_client =
            PubkyClient::get().map_err(|e| EventProcessorError::PubkyClientError {
                message: e.to_string(),
            })?;

        response = match pubky_client.public_storage().get(&pubkyapp_file.src).await {
            Ok(response) => response,
            Err(e) => {
                return Err(EventProcessorError::PubkyClientError {
                    message: format!(
                        "The ingest process could not get the client while processing File event: {e}"
                    ),
                }
                .into());
            }
        };
    }

    let path = Path::new(&user_id.to_string()).join(file_id);
    let full_path = files_path.join(path.clone());

    let blob = response.bytes().await?;
    let pubky_app_object = PubkyAppObject::from_uri(&pubkyapp_file.src, &blob)?;

    match pubky_app_object {
        PubkyAppObject::Blob(blob) => {
            Blob::put_to_static(FileVariant::Main.to_string(), full_path, &blob).await?;

            let urls = VariantController::get_file_urls_by_content_type(
                pubkyapp_file.content_type.as_str(),
                &path,
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

pub async fn del(user_id: &PubkyId, file_id: String, files_path: PathBuf) -> Result<(), DynError> {
    debug!("Deleting File resource at {}/{}", user_id, file_id);
    let result = FileDetails::get_by_ids(
        vec![vec![user_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await?;

    if !result.is_empty() {
        let file = &result[0];

        if let Some(file_details) = file {
            file_details.delete().await.map_err(|e| match e {
                DbError::GraphQueryFailed { message } => {
                    EventProcessorError::GraphQueryFailed { message }
                }
                DbError::IndexOperationFailed { message } => {
                    EventProcessorError::IndexWriteFailed { message }
                }
            })?;
        }

        let folder_path = Path::new(&user_id.to_string()).join(&file_id);
        let full_path = files_path.join(folder_path);

        remove_dir_all(full_path).await?;
    }

    Ok(())
}
