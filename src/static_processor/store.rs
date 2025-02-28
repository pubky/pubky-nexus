use std::path::PathBuf;

use pubky_app_specs::PubkyAppBlob;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

use crate::types::DynError;

pub struct StaticStorage;

impl StaticStorage {
    pub async fn store_blob(
        name: String,
        files_path: PathBuf,
        blob: &PubkyAppBlob,
    ) -> Result<(), DynError> {
        if !fs::metadata(&files_path)
            .await
            .is_ok_and(|metadata| metadata.is_dir())
        {
            fs::create_dir_all(&files_path).await?;
        };

        let file_path = files_path.join(name);
        let mut static_file = File::create_new(file_path).await?;
        static_file.write_all(&blob.0).await?;

        Ok(())
    }
}
