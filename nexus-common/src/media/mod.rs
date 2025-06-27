use crate::{
    models::file::{FileDetails, FileUrls},
    types::DynError,
};
use processors::{ImageProcessor, VariantProcessor, VideoProcessor};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio::fs;
use utoipa::ToSchema;

pub mod processors;

#[derive(Debug, PartialEq, Serialize, Deserialize, ToSchema, Clone)]
#[serde(rename_all = "lowercase")]
pub enum FileVariant {
    Main,
    Feed,
    Small,
}

impl FromStr for FileVariant {
    type Err = DynError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "main" => Ok(FileVariant::Main),
            "feed" => Ok(FileVariant::Feed),
            "small" => Ok(FileVariant::Small),
            _ => Err("Invalid file version".into()),
        }
    }
}

impl Display for FileVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version_string = match self {
            FileVariant::Main => "main",
            FileVariant::Feed => "feed",
            FileVariant::Small => "small",
        };
        write!(f, "{version_string}")
    }
}

pub struct VariantController;

impl VariantController {
    pub async fn create_file_variant(
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

    pub async fn check_variant_exists(
        file: &FileDetails,
        variant: FileVariant,
        file_path: PathBuf,
    ) -> bool {
        // main variant always exists
        if variant == FileVariant::Main {
            return true;
        }

        // if file exists, variant has already been created
        let path = file_path
            .join(file.owner_id.as_str())
            .join(file.id.as_str())
            .join(variant.to_string());

        fs::metadata(path).await.is_ok()
    }

    pub fn get_content_type_for_variant(file: &FileDetails, variant: &FileVariant) -> String {
        match &file.content_type {
            content_type if content_type.starts_with("image/") => {
                ImageProcessor::get_content_type_for_variant(file, variant)
            }
            content_type if content_type.starts_with("video/") => {
                VideoProcessor::get_content_type_for_variant(file, variant)
            }
            _ => file.content_type.clone(),
        }
    }

    pub fn get_file_urls_by_content_type(content_type: &str, path: &Path) -> FileUrls {
        let variants = Self::get_valid_variants_for_content_type(content_type);

        FileUrls::new(path, &variants)
    }

    pub fn validate_variant_for_content_type(content_type: &str, variant: &FileVariant) -> bool {
        if variant == &FileVariant::Main {
            return true;
        }
        let valid_variants = Self::get_valid_variants_for_content_type(content_type);
        valid_variants.contains(variant)
    }

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
}
