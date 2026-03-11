use async_trait::async_trait;
use tokio::process::Command;

use crate::{
    media::{processors::MediaProcessorError, FileVariant},
    models::file::FileDetails,
};

use super::{BaseProcessingOptions, VariantProcessor};

pub struct VideoOptions {
    width: String,
    format: String,
    content_type: String,
}

impl BaseProcessingOptions for VideoOptions {
    fn content_type(&self) -> String {
        self.content_type.clone()
    }
}

/// VideoProcessor is just a prototype and not a real implementation
/// when we decide to actual start video processing we will need to implement this.
pub struct VideoProcessor;

#[async_trait]
impl VariantProcessor for VideoProcessor {
    type ProcessingOptions = VideoOptions;

    fn get_valid_variants_for_content_type(_content_type: &str) -> Vec<FileVariant> {
        vec![FileVariant::Main]
    }

    fn get_content_type_for_variant(file: &FileDetails, variant: &FileVariant) -> String {
        if variant.eq(&FileVariant::Main) {
            return file.content_type.clone();
        }
        String::from("video/mp4")
    }

    fn get_options_for_variant(
        _file: &FileDetails,
        _variant: &FileVariant,
    ) -> Result<VideoOptions, MediaProcessorError> {
        // Return Err until we have a real implementation
        // TODO: Add real implementation for videos
        Err(MediaProcessorError::NotImplemented)
    }

    async fn process(
        origin_file_path: &str,
        output_file_path: &str,
        options: &VideoOptions,
    ) -> Result<String, MediaProcessorError> {
        let origin_file_format = VideoProcessor::get_format(origin_file_path).await?;

        let output = match origin_file_format == options.format {
            true => output_file_path.to_string(),
            false => format!("{}.{}", output_file_path, options.format),
        };

        let child_output = Command::new("ffmpeg")
            .arg("-i")
            .arg(origin_file_path)
            .arg("-vf")
            .arg(format!("scale={}:-1", options.width))
            .arg("-c:a")
            .arg("copy")
            .arg(output)
            .output() // Automatically pipes stdout and stderr
            .await
            .map_err(MediaProcessorError::command_failed)?;

        if child_output.status.success() {
            Ok(String::from_utf8_lossy(&child_output.stdout).to_string())
        } else {
            Err(MediaProcessorError::command_failed(format!(
                "FFmpeg command failed: {}",
                String::from_utf8_lossy(&child_output.stderr)
            )))
        }
    }
}

impl VideoProcessor {
    /// Returns the format of the video
    async fn get_format(input: &str) -> Result<String, MediaProcessorError> {
        let child_output = Command::new("ffmpeg")
            .arg("-i")
            .arg(input)
            .arg("-f")
            .arg("null")
            .output() // Automatically pipes stdout and stderr
            .await
            .map_err(MediaProcessorError::command_failed)?;

        if child_output.status.success() {
            Ok(String::from_utf8_lossy(&child_output.stdout).to_string())
        } else {
            Err(MediaProcessorError::command_failed(format!(
                "FFmpeg metadata extraction failed: {}",
                String::from_utf8_lossy(&child_output.stderr)
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file(content_type: &str) -> FileDetails {
        FileDetails {
            content_type: content_type.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_main_variant_preserves_original_content_type() {
        let file = make_file("video/webm");
        let result = VideoProcessor::get_content_type_for_variant(&file, &FileVariant::Main);
        assert_eq!(result, "video/webm");
    }

    #[test]
    fn test_main_variant_preserves_mp4_content_type() {
        let file = make_file("video/mp4");
        let result = VideoProcessor::get_content_type_for_variant(&file, &FileVariant::Main);
        assert_eq!(result, "video/mp4");
    }
}
