use std::path::PathBuf;

use async_trait::async_trait;

use crate::{
    models::file::{details::FileVariant, FileDetails},
    types::DynError,
};

pub trait BaseProcessingOptions: Send + Sync {
    fn content_type(&self) -> String;
}

#[async_trait]
pub trait FileProcessor {
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
    ) -> Result<Self::ProcessingOptions, DynError>;

    /// Processes the origin file and saves the output to the output_file_path based on the passed options
    /// Returns the content type of the processed file or the original content type if no processing was done
    async fn process(
        origin_file_path: &str,
        output_file_path: &str,
        options: &Self::ProcessingOptions,
    ) -> Result<String, DynError>;

    /// Creates a variant for the given file
    /// If there are no options for this variant, return with the original content type
    async fn create_variant(
        file: &FileDetails,
        variant: &FileVariant,
        file_path: PathBuf,
    ) -> Result<String, DynError> {
        // if there are no options for this variant, return with the original content type
        let options = match Self::get_options_for_variant(file, variant) {
            Ok(options) => options,
            Err(_) => return Ok(file.content_type.clone()),
        };

        let origin_path = file_path
            .join(file.owner_id.as_str())
            .join(file.id.as_str());

        let origin_file = origin_path.join(FileVariant::Main.to_string());

        let origin_file_path = match origin_file.to_str() {
            Some(path) => path,
            None => return Err("Invalid original file path".into()),
        };

        let output = origin_path.join(variant.to_string());
        let output_path = match output.to_str() {
            Some(path) => path,
            None => return Err("Invalid output path".into()),
        };

        Self::process(origin_file_path, output_path, &options).await?;

        Ok(options.content_type())
    }
}
