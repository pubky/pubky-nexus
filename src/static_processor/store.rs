use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

use crate::{types::DynError, Config};

pub struct StaticStorage;

impl StaticStorage {
    pub async fn store_blob(name: String, path: String, blob: &[u8]) -> Result<(), DynError> {
        if !fs::metadata(path.as_str())
            .await
            .is_ok_and(|metadata| metadata.is_dir())
        {
            fs::create_dir_all(path.as_str()).await?;
        };

        let file_path = format!("{}/{}", path, name);
        let mut static_file = File::create_new(file_path).await?;
        static_file.write_all(blob).await?;

        Ok(())
    }

    pub fn get_storage_path() -> String {
        Config::from_env().file_path
    }
}
