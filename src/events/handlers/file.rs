use std::error::Error;

use axum::body::Bytes;
use chrono::Utc;
use log::debug;
use pubky::PubkyClient;
use tokio::{fs::File, io::AsyncWriteExt};

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
    let file_input = <PubkyAppFile as Validatable>::try_from(&blob).await?;

    // Create FileDetails object
    let file_details = from_homeserver(uri, user_id, file_id, file_input, client).await?;

    // Index new user event into the Graph and Index
    file_details.save().await?;

    Ok(())
}

async fn from_homeserver(
    uri: String,
    user_id: PubkyId,
    file_id: String,
    pubkyapp_file: PubkyAppFile,
    client: &PubkyClient,
) -> Result<FileDetails, Box<dyn std::error::Error + Send + Sync>> {
    let file_meta = ingest(&user_id, file_id.as_str(), &pubkyapp_file, client).await?;

    Ok(FileDetails {
        name: pubkyapp_file.name,
        src: pubkyapp_file.src,
        content_type: pubkyapp_file.content_type,
        uri: uri,
        id: file_id,
        created_at: Utc::now().timestamp_millis(),
        indexed_at: Utc::now().timestamp_millis(),
        owner_id: user_id.to_string(),
        size: pubkyapp_file.size,
        urls: FileUrls {
            main: file_meta.urls.main,
        },
    })
}

// TODO: Move it into its own process, server, etc
async fn ingest(
    user_id: &PubkyId,
    file_id: &str,
    pubkyapp_file: &PubkyAppFile,
    client: &PubkyClient,
) -> Result<FileMeta, Box<dyn std::error::Error + Send + Sync>> {
    let static_path = format!("{}/{}", user_id, file_id);

    let response = client.get(pubkyapp_file.src.as_str()).await?.unwrap();

    let mut static_file = File::create(format!("static/files/{}", &static_path)).await?;

    static_file.write_all(&response).await?;

    Ok(FileMeta {
        urls: FileUrls { main: static_path },
    })
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

    Ok(())
}

// Parses a file id from the event's uri
pub fn parse_file_id(uri: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Define the pattern we're looking for in the URI
    let file_pattern: &str = "/files/";

    // Find the starting position of the file_id part in the URI
    let start_idx = uri
        .find(file_pattern)
        .map(|start| start + file_pattern.len())
        .ok_or("File pattern not found in URI")?;

    // Extract the file_id from the path
    let file_id = &uri[start_idx..];

    // Return the post_id as a string
    Ok(file_id.to_string())
}
