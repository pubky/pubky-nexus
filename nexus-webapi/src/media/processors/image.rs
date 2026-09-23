use async_trait::async_trait;
use tokio::process::Command;

use crate::media::{processors::MediaProcessorError, MediaSubprocess};
use nexus_common::media::FileVariant;

use super::{BaseProcessingOptions, VariantProcessor};

const SMALL_IMAGE_WIDTH: &str = "320";
const FEED_IMAGE_WIDTH: &str = "720";
/// Full-width cover on wide screens.
const LARGE_IMAGE_WIDTH: &str = "1440";
/// The format `process` hands ImageMagick as its output format, i.e. the bytes a derived variant
/// actually contains.
const IMAGE_FORMAT: &str = "webp";

/// The content type a derived image variant is served under. Derived from [`IMAGE_FORMAT`] rather
/// than written out, so the label cannot name one format while the converter writes another.
pub(crate) fn image_variant_content_type() -> String {
    format!("image/{IMAGE_FORMAT}")
}

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

/// The `-resize` geometry for a derived variant: fit within `width`, never enlarge (`>`).
fn resize_geometry(width: &str) -> String {
    format!("{}x>", width)
}

pub struct ImageProcessor;

#[async_trait]
impl VariantProcessor for ImageProcessor {
    type ProcessingOptions = ImageOptions;

    fn get_options_for_variant(variant: &FileVariant) -> Result<ImageOptions, MediaProcessorError> {
        let width = match variant {
            FileVariant::Small => String::from(SMALL_IMAGE_WIDTH),
            FileVariant::Feed => String::from(FEED_IMAGE_WIDTH),
            FileVariant::Large => String::from(LARGE_IMAGE_WIDTH),
            _ => return Err(MediaProcessorError::UnsupportedFileVariant),
        };
        // `variant` is Small, Feed or Large here: Main returned above.
        let content_type = image_variant_content_type();
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
        subprocess: MediaSubprocess,
    ) -> Result<String, MediaProcessorError> {
        let origin_file_format = ImageProcessor::get_format(origin_file_path, subprocess)
            .await?
            .to_lowercase();

        let output = match origin_file_format == options.format {
            true => output_file_path.to_string(),
            false => format!("{}:{}", options.format, output_file_path),
        };

        let child_output = subprocess
            .run(
                Command::new("convert")
                    .arg(origin_file_path)
                    .arg("-resize")
                    .arg(resize_geometry(&options.width))
                    .arg("-auto-orient") // https://github.com/ImageMagick/ImageMagick/issues/6396
                    .arg(output),
            )
            .await?;

        if child_output.status.success() {
            Ok(String::from_utf8_lossy(&child_output.stdout).to_string())
        } else {
            Err(MediaProcessorError::command_failed(format!(
                "ImageMagick command failed: {}",
                String::from_utf8_lossy(&child_output.stderr)
            )))
        }
    }
}

impl ImageProcessor {
    // function to get image format
    async fn get_format(
        file_path: &str,
        subprocess: MediaSubprocess,
    ) -> Result<String, MediaProcessorError> {
        let child_output = subprocess
            .run(
                Command::new("identify")
                    .arg("-format")
                    .arg("%m")
                    .arg(file_path),
            )
            .await?;

        if child_output.status.success() {
            Ok(String::from_utf8_lossy(&child_output.stdout).to_string())
        } else {
            Err(MediaProcessorError::command_failed(format!(
                "ImageMagick format extraction failed: {}",
                String::from_utf8_lossy(&child_output.stderr)
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The label and the bytes are one fact. If `IMAGE_FORMAT` changes, this fails and whoever
    // changed it has to confirm the served content type is meant to change with it.
    #[test]
    fn test_variant_content_type_tracks_the_output_format() {
        assert_eq!(IMAGE_FORMAT, "webp");
        assert_eq!(image_variant_content_type(), "image/webp");
    }

    // One assertion per variant, so a width cannot change, or a variant appear, unlisted.
    #[test]
    fn test_resize_geometry_never_enlarges() {
        assert_eq!(resize_geometry("320"), "320x>");
        assert_eq!(resize_geometry("720"), "720x>");
        assert_eq!(resize_geometry("1440"), "1440x>");
    }

    #[test]
    fn test_variant_widths() {
        let width = |variant: FileVariant| {
            ImageProcessor::get_options_for_variant(&variant)
                .expect("variant has image options")
                .width
        };

        assert_eq!(width(FileVariant::Small), "320");
        assert_eq!(width(FileVariant::Feed), "720");
        assert_eq!(width(FileVariant::Large), "1440");
        assert!(matches!(
            ImageProcessor::get_options_for_variant(&FileVariant::Main),
            Err(MediaProcessorError::UnsupportedFileVariant)
        ));
    }
}
