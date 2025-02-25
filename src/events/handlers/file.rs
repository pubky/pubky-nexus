use crate::db::connectors::pubky::PubkyConnector;
use crate::events::error::EventProcessorError;
use crate::types::DynError;
use crate::{
    models::{
        file::{
            details::{FileMeta, FileUrls},
            FileDetails,
        },
        traits::Collection,
    },
    Config,
};
use pubky_app_specs::{PubkyAppFile, PubkyAppObject, PubkyId};
use tokio::{
    fs::{self, remove_file, File},
    io::AsyncWriteExt,
};
use tracing::{debug, error};

pub async fn sync_put(
    file: PubkyAppFile,
    uri: String,
    user_id: PubkyId,
    file_id: String,
) -> Result<(), DynError> {
    debug!("Indexing new file resource at {}/{}", user_id, file_id);

    debug!("file input {:?}", file);

    let file_meta = ingest(&user_id, file_id.as_str(), &file).await?;

    // Create FileDetails object
    let file_details =
        FileDetails::from_homeserver(&file, uri, user_id.to_string(), file_id, file_meta);

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
) -> Result<FileMeta, DynError> {
    let response;
    {
        let pubky_client = PubkyConnector::get_pubky_client().await?;

        response = match pubky_client.get(&pubkyapp_file.src).send().await {
            Ok(response) => response,
            // TODO: Shape the error to avoid the retyManager
            Err(e) => {
                error!("EVENT ERROR: could not retrieve file src blob");
                return Err(e.into());
            }
        };
    }

    let blob = response.bytes().await?;
    let pubky_app_object = PubkyAppObject::from_uri(&pubkyapp_file.src, &blob)?;

    match pubky_app_object {
        PubkyAppObject::Blob(blob) => {
            store_blob(file_id.to_string(), user_id.to_string(), &blob.0).await?;

            Ok(FileMeta {
                urls: FileUrls {
                    main: format!("{}/{}", user_id, file_id),
                },
            })
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

async fn store_blob(name: String, path: String, blob: &[u8]) -> Result<(), DynError> {
    let storage_path = Config::from_env().file_path;
    // TODO: Is it well formatting. The file path already has / at the end
    let full_path = format!("{}/{}", storage_path, path);

    debug!("store blob in full_path: {}", full_path);

    let path_exists = match fs::metadata(full_path.as_str()).await {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };

    debug!("path exists: {}", path_exists);

    if !path_exists {
        fs::create_dir_all(full_path.as_str()).await?;
    }

    let file_path = format!("{}/{}", full_path, name);
    let mut static_file = File::create_new(file_path).await?;
    static_file.write_all(blob).await?;

    Ok(())
}

async fn remove_blob(name: String, path: String) -> Result<(), DynError> {
    let storage_path = Config::from_env().file_path;
    let file_path = format!("{}/{}/{}", storage_path, path, name);

    remove_file(file_path).await?;
    Ok(())
}

pub async fn del(user_id: &PubkyId, file_id: String) -> Result<(), DynError> {
    debug!("Deleting File resource at {}/{}", user_id, file_id);
    let result = FileDetails::get_by_ids(
        vec![vec![user_id.as_str(), file_id.as_str()].as_slice()].as_slice(),
    )
    .await?;

    if !result.is_empty() {
        let file = &result[0];

        if let Some(value) = file {
            value.delete().await?;
        }
        remove_blob(file_id, user_id.to_string()).await?;
    }

    Ok(())
}
