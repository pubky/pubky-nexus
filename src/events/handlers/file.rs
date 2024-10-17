use std::error::Error;

use axum::body::Bytes;
use log::debug;
use pubky::PubkyClient;
use tokio::fs;

use crate::{
    models::{
        file::{
            details::{FileMeta, FileUrls},
            FileDetails,
        },
        pubky_app::{traits::Validatable, PubkyAppFile},
        traits::Collection,
        user::PubkyId,
    },
    static_processor::store::{remove_blob, store_blob},
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

    debug!("file input {:?}", file_input);

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

    let path = format!("{}/{}", user_id, file_id);
    store_blob(String::from("main"), path.to_string(), &response).await?;

    let main_static_path = format!("{}/main", path);
    Ok(FileMeta {
        urls: FileUrls {
            main: main_static_path,
        },
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

    let folder_path = format!("{}/{}", user_id, file_id);

    let mut directory = fs::read_dir(folder_path.clone()).await?;

    loop {
        let entry = directory.next_entry().await?;
        match entry {
            Some(path) => {
                remove_blob(
                    String::from(path.file_name().to_str().unwrap()),
                    folder_path.clone(),
                )
                .await?
            }
            None => break,
        };
    }
    Ok(())
}
