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
        let mut static_file = File::create_new(file_path).await?;
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
