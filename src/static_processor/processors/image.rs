use axum::async_trait;
use tokio::process::Command;

use crate::{
    models::file::{details::FileVariant, FileDetails},
    types::DynError,
};

use super::traits::{BaseProcessingOptions, FileProcessor};

const SMALL_IMAGE_WIDTH: &str = "320";
const FEED_IMAGE_WIDTH: &str = "720";

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
impl FileProcessor for ImageProcessor {
    type ProcessingOptions = ImageOptions;

    fn get_valid_variants_for_content_type(content_type: &str) -> Vec<FileVariant> {
        match content_type {
            value if value.ends_with("gif") => vec![FileVariant::Main],
            _ => vec![FileVariant::Main, FileVariant::Small, FileVariant::Feed],
        }
    }

    fn get_options_for_variant(
        file: &FileDetails,
        variant: &FileVariant,
    ) -> Result<ImageOptions, DynError> {
        let format = match file.content_type.as_str() {
            "image/gif" => String::from("gif"),
            _ => String::from("jpeg"),
        };
        let width = match variant {
            FileVariant::Small => String::from(SMALL_IMAGE_WIDTH),
            FileVariant::Feed => String::from(FEED_IMAGE_WIDTH),
            _ => return Err("Unsupported image variant".into()),
        };
        let content_type = match file.content_type.as_str() {
            "image/gif" => file.content_type.clone(),
            _ => String::from("image/jpeg"),
        };
        Ok(ImageOptions {
            format,
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
            .arg("-background")
            .arg("white")
            .arg("-alpha")
            .arg("remove")
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
