use crate::{media::FileVariant, models::file::FileDetails};
use async_trait::async_trait;
use std::path::PathBuf;
use thiserror::Error;

mod image;
mod video;

pub use image::*;
pub use video::*;

pub trait BaseProcessingOptions: Send + Sync {
    fn content_type(&self) -> String;
}

#[derive(Error, Debug)]
pub enum MediaProcessorError {
    #[error("CommandFailed: {source}")]
    CommandFailed {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("NotImplemented")]
    NotImplemented,
    #[error("UnsupportedContentType: {0}")]
    UnsupportedContentType(String),
    #[error("UnsupportedFileVariant")]
    UnsupportedFileVariant,
    #[error("InvalidFilePath: {0}")]
    InvalidFilePath(String),
}

impl MediaProcessorError {
    pub fn command_failed(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::CommandFailed {
            source: source.into(),
        }
    }
}

#[async_trait]
pub trait VariantProcessor {
    type ProcessingOptions: BaseProcessingOptions;

    /// Returns a list of valid variants for a given content type
    /// If there are no valid variants for the content type, return an empty list
    fn get_valid_variants_for_content_type(content_type: &str) -> Vec<FileVariant>;

    /// Returns the content type for a given variant
    fn get_content_type_for_variant(file: &FileDetails, variant: &FileVariant) -> String;

    /// Returns the processing options for a given variant
    /// If there are no options for this variant, return an error
    fn get_options_for_variant(
        file: &FileDetails,
        variant: &FileVariant,
    ) -> Result<Self::ProcessingOptions, MediaProcessorError>;

    /// Processes the origin file and saves the output to the output_file_path based on the passed options
    /// Returns the content type of the processed file or the original content type if no processing was done
    async fn process(
        origin_file_path: &str,
        output_file_path: &str,
        options: &Self::ProcessingOptions,
    ) -> Result<String, MediaProcessorError>;

    /// Creates a variant for the given file
    /// If there are no options for this variant, return with the original content type
    async fn create_variant(
        file: &FileDetails,
        variant: &FileVariant,
        file_path: PathBuf,
    ) -> Result<String, MediaProcessorError> {
        // if there are no options for this variant, return with the original content type
        let options = match Self::get_options_for_variant(file, variant) {
            Ok(options) => options,
            Err(_) => return Ok(file.content_type.clone()),
        };

        let origin_path = file_path
            .join(file.owner_id.as_str())
            .join(file.id.as_str());

        let origin_file = origin_path.join(FileVariant::Main.to_string());

        let Some(origin_file_path) = origin_file.to_str() else {
            return Err(MediaProcessorError::InvalidFilePath(
                "Original file".to_string(),
            ));
        };

        let output = origin_path.join(variant.to_string());
        let Some(output_path) = output.to_str() else {
            return Err(MediaProcessorError::InvalidFilePath(
                "Output file".to_string(),
            ));
        };

        Self::process(origin_file_path, output_path, &options).await?;

        Ok(options.content_type())
    }
}
