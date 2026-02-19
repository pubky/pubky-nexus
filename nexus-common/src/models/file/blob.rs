use crate::media::{
    processors::{ImageProcessor, VariantProcessor, VideoProcessor},
    FileVariant, VariantController,
};
use crate::types::DynError;
use pubky_app_specs::PubkyAppBlob;
use std::path::PathBuf;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};
use tracing::error;

use super::FileDetails;

pub struct Blob;

impl Blob {
    pub async fn put_to_static(
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
        let mut static_file = File::create(file_path).await?;
        static_file.write_all(&blob.0).await?;

        Ok(())
    }

    pub async fn get_by_id(
        file: &FileDetails,
        variant: &FileVariant,
        file_path: PathBuf,
    ) -> Result<String, DynError> {
        let file_variant_exists =
            VariantController::check_variant_exists(file, variant.clone(), file_path.clone()).await;

        if file_variant_exists {
            Ok(VariantController::get_content_type_for_variant(
                file, variant,
            ))
        } else {
            match Self::put_variant(file, variant, file_path).await {
                Ok(content_type) => Ok(content_type),
                Err(err) => {
                    error!(
                        "Creating variant failed for file: {:?} with error: {}",
                        file, err
                    );
                    Err(err)
                }
            }
        }
    }

    async fn put_variant(
        file: &FileDetails,
        variant: &FileVariant,
        file_path: PathBuf,
    ) -> Result<String, DynError> {
        match &file.content_type {
            content_type if content_type.starts_with("image/") => {
                ImageProcessor::create_variant(file, variant, file_path).await
            }
            content_type if content_type.starts_with("video/") => {
                VideoProcessor::create_variant(file, variant, file_path).await
            }
            _ => Err(format!("Unsupported content type: {}", file.content_type).into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky_app_specs::PubkyAppBlob;
    use tokio::io::AsyncReadExt;

    #[tokio_shared_rt::test(shared)]
    async fn test_put_to_static_creates_new_file() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let files_path = tmp_dir.path().join("user1").join("file1");
        let blob = PubkyAppBlob::new(b"hello world".to_vec());

        Blob::put_to_static("main".to_string(), files_path.clone(), &blob)
            .await
            .expect("put_to_static should succeed for a new file");

        let mut content = Vec::new();
        File::open(files_path.join("main"))
            .await
            .unwrap()
            .read_to_end(&mut content)
            .await
            .unwrap();
        assert_eq!(content, b"hello world");
    }

    #[tokio_shared_rt::test(shared)]
    async fn test_put_to_static_overwrites_existing_file() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let files_path = tmp_dir.path().join("user1").join("file1");
        let blob1 = PubkyAppBlob::new(b"first write".to_vec());

        Blob::put_to_static("main".to_string(), files_path.clone(), &blob1)
            .await
            .expect("first put_to_static should succeed");

        // Calling put_to_static again simulates re-indexing when the file already exists on disk
        let blob2 = PubkyAppBlob::new(b"second write".to_vec());
        Blob::put_to_static("main".to_string(), files_path.clone(), &blob2)
            .await
            .expect("put_to_static should succeed even when file already exists (re-indexing)");

        let mut content = Vec::new();
        File::open(files_path.join("main"))
            .await
            .unwrap()
            .read_to_end(&mut content)
            .await
            .unwrap();
        assert_eq!(content, b"second write");
    }
}
