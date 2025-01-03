use std::path::PathBuf;

use tokio::fs;

use crate::{
    models::file::{
        details::{FileUrls, FileVariant},
        FileDetails,
    },
    types::DynError,
};

use super::{
    processors::{image::ImageProcessor, traits::FileProcessor, video::VideoProcessor},
    StaticStorage,
};

pub struct StaticProcessor;

impl StaticProcessor {
    fn get_valid_variants_for_content_type(content_type: &str) -> Vec<FileVariant> {
        match content_type {
            value if value.starts_with("image") => {
                ImageProcessor::get_valid_variants_for_content_type(content_type)
            }
            value if value.starts_with("video") => {
                VideoProcessor::get_valid_variants_for_content_type(content_type)
            }
            _ => vec![],
        }
    }

    pub fn validate_variant_for_content_type(content_type: &str, variant: FileVariant) -> bool {
        if variant == FileVariant::Main {
            return true;
        }
        let valid_variants = Self::get_valid_variants_for_content_type(content_type);
        valid_variants.contains(&variant)
    }

    pub fn get_file_urls_by_content_type(content_type: &str, path: &str) -> FileUrls {
        let variants = Self::get_valid_variants_for_content_type(content_type);

        FileUrls {
            main: format!("{}/main", path),
            feed: variants
                .contains(&FileVariant::Feed)
                .then_some(format!("{}/feed", path)),
            small: variants
                .contains(&FileVariant::Small)
                .then_some(format!("{}/small", path)),
        }
    }

    pub async fn create_file_variant(
        file: &FileDetails,
        variant: FileVariant,
    ) -> Result<String, DynError> {
        match &file.content_type {
            content_type if content_type.starts_with("image/") => {
                ImageProcessor::create_variant(file, variant).await
            }
            content_type if content_type.starts_with("video/") => {
                VideoProcessor::create_variant(file, variant).await
            }
            _ => Err(format!("Unsupported content type: {}", file.content_type).into()),
        }
    }

    pub async fn check_variant_existence(file: &FileDetails, variant: FileVariant) -> bool {
        // main variant always exists
        if variant == FileVariant::Main {
            return true;
        }

        // if file exists, variant has already been created
        let path = PathBuf::from(StaticStorage::get_storage_path())
            .join(file.owner_id.as_str())
            .join(file.id.as_str())
            .join(variant.to_string());

        fs::metadata(path).await.is_ok()
    }
}
