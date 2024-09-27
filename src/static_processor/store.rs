use axum::body::Bytes;
use tokio::{
    fs::{self, remove_file, File},
    io::AsyncWriteExt,
};

use crate::{types::DynError, Config};

pub async fn store_blob(name: String, path: String, blob: &Bytes) -> Result<(), DynError> {
    let storage_path = get_storage_path();
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

pub async fn remove_blob(name: String, path: String) -> Result<(), DynError> {
    let storage_path = get_storage_path();
    let file_path = format!("{}/{}/{}", storage_path, path, name);

    remove_file(file_path).await?;
    Ok(())
}

pub fn get_storage_path() -> String {
    Config::from_env().file_path
}
