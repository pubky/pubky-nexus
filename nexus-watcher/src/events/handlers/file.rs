use crate::events::{fetch_capped, EventProcessorError};

use nexus_common::db::PubkyConnector;
use nexus_common::media::FileVariant;
use nexus_common::media::VariantController;
use nexus_common::models::file::Blob;
use nexus_common::models::user::UserIngestor;
use nexus_common::models::{
    file::{FileDetails, FileMeta},
    traits::Collection,
};
use pubky_app_specs::{ParsedUri, PubkyAppFile, PubkyAppObject, PubkyId};
use std::path::Path;
use tokio::fs::remove_dir_all;
use tracing::{debug, warn};

#[tracing::instrument(name = "file.put", skip_all, fields(user_id = %user_id, file_id = %file_id))]
pub async fn sync_put(
    file: PubkyAppFile,
    uri: String,
    user_id: PubkyId,
    file_id: String,
    files_path: &Path,
    max_file_size: u64,
    ingestor: &UserIngestor,
) -> Result<(), EventProcessorError> {
    debug!("Indexing new file resource at {}/{}", user_id, file_id);

    let file_meta = ingest(
        &user_id,
        file_id.as_str(),
        &file,
        files_path,
        max_file_size,
        ingestor,
    )
    .await?;

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
#[tracing::instrument(name = "file.ingest", skip_all, fields(user_id = %user_id, file_id = %file_id))]
async fn ingest(
    user_id: &PubkyId,
    file_id: &str,
    pubkyapp_file: &PubkyAppFile,
    files_path: &Path,
    max_file_size: u64,
    ingestor: &UserIngestor,
) -> Result<FileMeta, EventProcessorError> {
    let file_src = &pubkyapp_file.src;
    let parsed_source_uri = ParsedUri::try_from(file_src.to_string()).map_err(|e| {
        EventProcessorError::generic(format!("Invalid file source URI {file_src}: {e}"))
    })?;

    // Refuse to download content hosted on a blacklisted HS
    ingestor
        .ensure_hs_not_blacklisted(&parsed_source_uri.user_id)
        .await
        .inspect_err(|e| warn!("Aborting file ingest: source {file_src}: {e}"))?;

    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(&pubkyapp_file.src).await?;

    let path = Path::new(&user_id.to_string()).join(file_id);
    let full_path = files_path.join(path.clone());

    let blob = fetch_capped(response, max_file_size).await?;
    let pubky_app_object = PubkyAppObject::from_resource(&parsed_source_uri.resource, &blob)
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

#[tracing::instrument(name = "file.del", skip_all, fields(user_id = %user_id, file_id = %file_id))]
pub async fn del(
    user_id: &PubkyId,
    file_id: String,
    files_path: &Path,
) -> Result<(), EventProcessorError> {
    debug!("Deleting File resource at {}/{}", user_id, file_id);
    let result = FileDetails::get_by_ids(&[&[user_id, &file_id]]).await?;

    if result.is_empty() {
        return Ok(());
    }

    let file = &result[0];
    if let Some(file_details) = file {
        file_details.delete().await?;
    }

    let folder_path = Path::new(&user_id.to_string()).join(&file_id);
    let full_path = files_path.join(folder_path);

    match remove_dir_all(full_path.as_path()).await {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
