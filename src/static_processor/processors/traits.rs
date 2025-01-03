use std::path::PathBuf;

use axum::async_trait;

use crate::{
    models::file::{details::FileVariant, FileDetails},
    static_processor::StaticStorage,
    types::DynError,
};

pub trait BaseProcessingOptions: Send + Sync {
    fn content_type(&self) -> String;
}

#[async_trait]
pub trait FileProcessor {
    type ProcessingOptions: BaseProcessingOptions;

    fn get_valid_variants_for_content_type(content_type: &str) -> Vec<FileVariant>;
    fn get_options_for_variant(
        file: &FileDetails,
        variant: &FileVariant,
    ) -> Result<Self::ProcessingOptions, DynError>;
    async fn process(
        origin_file_path: &str,
        output_file_path: &str,
        options: &Self::ProcessingOptions,
    ) -> Result<String, DynError>;

    async fn create_variant(file: &FileDetails, variant: FileVariant) -> Result<String, DynError> {
        // if there are no options for this variant, return with the original content type
        let options = match Self::get_options_for_variant(file, &variant) {
            Ok(options) => options,
            Err(_) => return Ok(file.content_type.clone()),
        };

        let origin_path = PathBuf::from(StaticStorage::get_storage_path())
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

        match Self::process(origin_file_path, output_path, &options).await {
            Ok(_) => Ok(options.content_type()),
            Err(err) => Err(format!("processing failed: {}", err).into()),
        }
    }
}
