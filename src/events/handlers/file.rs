use std::{env::current_dir, error::Error};

use axum::body::Bytes;
use log::debug;
use pubky::PubkyClient;
use tokio::{
    fs::{self, remove_file, File},
    io::AsyncWriteExt,
};

use crate::models::{
    file::{
        details::{FileMeta, FileUrls},
        FileDetails,
    },
    pubky_app::{traits::Validatable, PubkyAppFile},
    traits::Collection,
    user::PubkyId,
};

pub async fn put(
    uri: String,
    user_id: PubkyId,
    file_id: String,
    blob: Bytes,
    client: &PubkyClient,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Indexing new file resource at {}/{}", user_id, file_id);

    // Serialize and validate
    let file_input = <PubkyAppFile as Validatable>::try_from(&blob, &file_id).await?;

    let file_meta = ingest(&user_id, file_id.as_str(), &file_input, client).await?;

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
    client: &PubkyClient,
) -> Result<FileMeta, Box<dyn std::error::Error + Send + Sync>> {
    let response = client.get(pubkyapp_file.src.as_str()).await?.unwrap();

    store_blob(file_id.to_string(), user_id.to_string(), &response).await?;

    let static_path = format!("{}/{}", user_id, file_id);
    Ok(FileMeta {
        urls: FileUrls { main: static_path },
    })
}

async fn store_blob(
    name: String,
    path: String,
    blob: &Bytes,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let storage_path = format!("{}/static/files", current_dir()?.display());
    let full_path = format!("{}/{}", storage_path, path);

    let path_exists = match fs::metadata(full_path.as_str()).await {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };

    if !path_exists {
        fs::create_dir_all(full_path.as_str()).await?;
    }

    let file_path = format!("{}/{}", full_path, name);
    let mut static_file = File::create_new(file_path).await?;
    static_file.write_all(blob).await?;

    Ok(())
}

async fn remove_blob(
    name: String,
    path: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let storage_path = format!("{}/static/files", current_dir()?.display());
    let file_path = format!("{}/{}/{}", storage_path, path, name);

    remove_file(file_path).await?;
    Ok(())
}

pub async fn del(user_id: &PubkyId, file_id: String) -> Result<(), Box<dyn Error + Sync + Send>> {
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
