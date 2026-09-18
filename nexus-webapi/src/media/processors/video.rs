use async_trait::async_trait;
use tokio::process::Command;

use crate::media::{processors::MediaProcessorError, MediaSubprocess};
use nexus_common::media::FileVariant;

use super::{BaseProcessingOptions, VariantProcessor};

/// The container `process` passes to ffmpeg as `-f`. Unused until `get_options_for_variant` is
/// implemented, which must set `VideoOptions::format` from it so the label keeps matching the
/// bytes.
const VIDEO_FORMAT: &str = "mp4";

/// The content type a derived video variant is served under. Derived from [`VIDEO_FORMAT`] for
/// the same reason as the image side.
pub(crate) fn video_variant_content_type() -> String {
    format!("video/{VIDEO_FORMAT}")
}

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

    fn get_options_for_variant(
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
        subprocess: MediaSubprocess,
    ) -> Result<String, MediaProcessorError> {
        // The caller renames this path into place, so write exactly here. The output carries no
        // extension for ffmpeg to infer the container from, hence the explicit `-f`.
        let child_output = subprocess
            .run(
                Command::new("ffmpeg")
                    .arg("-i")
                    .arg(origin_file_path)
                    .arg("-vf")
                    .arg(format!("scale={}:-1", options.width))
                    .arg("-c:a")
                    .arg("copy")
                    .arg("-f")
                    .arg(&options.format)
                    .arg(output_file_path),
            )
            .await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_content_type_tracks_the_output_format() {
        assert_eq!(VIDEO_FORMAT, "mp4");
        assert_eq!(video_variant_content_type(), "video/mp4");
    }
}
