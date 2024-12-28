use crate::db::connectors::pubky::PubkyConnector;
use crate::types::DynError;
use crate::types::PubkyId;
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
use axum::body::Bytes;
use pubky_app_specs::{traits::Validatable, PubkyAppFile};
use tokio::{
    fs::{self, remove_file, File},
    io::AsyncWriteExt,
};
use tracing::debug;

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
        None => return Err("EVENT ERROR: no metadata in the file blob".into()),
    };

    debug!("File Metadata: {:?}\n{:?}", file_id, blob);
    store_blob(file_id.to_string(), user_id.to_string(), &blob).await?;

    let static_path = format!("{}/{}", user_id, file_id);
    Ok(FileMeta {
        urls: FileUrls { main: static_path },
    })
}

async fn store_blob(name: String, path: String, blob: &Bytes) -> Result<(), DynError> {
    let storage_path = Config::from_env().file_path;
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

    let file = &result[0];

    if let Some(value) = file {
        value.delete().await?;
    }

    remove_blob(file_id, user_id.to_string()).await?;

    Ok(())
}
