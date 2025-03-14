use async_trait::async_trait;
use tokio::process::Command;

use crate::{media::FileVariant, models::file::FileDetails, types::DynError};

use super::{BaseProcessingOptions, VariantProcessor};

const SMALL_IMAGE_WIDTH: &str = "320";
const FEED_IMAGE_WIDTH: &str = "720";
const IMAGE_FORMAT: &str = "webp";

pub struct ImageOptions {
    width: String,
    format: String,
    content_type: String,
}

impl BaseProcessingOptions for ImageOptions {
    fn content_type(&self) -> String {
        self.content_type.clone()
    }
}

pub struct ImageProcessor;

#[async_trait]
impl VariantProcessor for ImageProcessor {
    type ProcessingOptions = ImageOptions;

    fn get_valid_variants_for_content_type(_content_type: &str) -> Vec<FileVariant> {
        vec![FileVariant::Main, FileVariant::Small, FileVariant::Feed]
    }

    fn get_content_type_for_variant(file: &FileDetails, variant: &FileVariant) -> String {
        if variant.eq(&FileVariant::Main) {
            return file.content_type.clone();
        }
        String::from("image/webp")
    }

    fn get_options_for_variant(
        file: &FileDetails,
        variant: &FileVariant,
    ) -> Result<ImageOptions, DynError> {
        let width = match variant {
            FileVariant::Small => String::from(SMALL_IMAGE_WIDTH),
            FileVariant::Feed => String::from(FEED_IMAGE_WIDTH),
            _ => return Err("Unsupported image variant".into()),
        };
        let content_type = Self::get_content_type_for_variant(file, variant);
        Ok(ImageOptions {
            format: IMAGE_FORMAT.to_string(),
            width,
            content_type,
        })
    }

    async fn process(
        origin_file_path: &str,
        output_file_path: &str,
        options: &ImageOptions,
    ) -> Result<String, DynError> {
        let origin_file_format = ImageProcessor::get_format(origin_file_path)
            .await?
            .to_lowercase();

        let output = match origin_file_format == options.format {
            true => output_file_path.to_string(),
            false => format!("{}:{}", options.format, output_file_path),
        };

        let child_output = Command::new("convert")
            .arg(origin_file_path)
            .arg("-resize")
            .arg(format!("{}x", options.width))
            .arg(output)
            .output() // Automatically pipes stdout and stderr
            .await?;

        if child_output.status.success() {
            Ok(String::from_utf8_lossy(&child_output.stdout).to_string())
        } else {
            Err(format!(
                "ImageMagick command failed: {}",
                String::from_utf8_lossy(&child_output.stdout)
            )
            .into())
        }
    }
}

impl ImageProcessor {
    // function to get image format
    async fn get_format(file_path: &str) -> Result<String, DynError> {
        let child_output = match Command::new("identify")
            .arg("-format")
            .arg("%m")
            .arg(file_path)
            .output() // Automatically pipes stdout and stderr
            .await
        {
            Ok(output) => output,
            Err(err) => return Err(err.into()),
        };

        if child_output.status.success() {
            Ok(String::from_utf8_lossy(&child_output.stdout).to_string())
        } else {
            Err(format!(
                "ImageMagick format extraction failed: {}",
                String::from_utf8_lossy(&child_output.stderr)
            )
            .into())
        }
    }
}
