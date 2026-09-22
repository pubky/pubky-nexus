//! Deriving file variants on demand: the concurrency gates that bound it, the subprocess runner
//! it goes through, and the per-format processors that do the converting.
//!
//! Only the API derives variants — a request for one that isn't on disk yet makes it — so the
//! machinery lives here, and so does the content type a derived variant is served under: that
//! label names what a processor writes, so it belongs beside the processor. The variant names and
//! the table of which variants a content type has are shared with the watcher and stay in
//! [`nexus_common::media`].

use std::{path::Path, sync::Arc};

use nexus_common::media::FileVariant;
use nexus_common::models::file::FileDetails;
use processors::{
    image_variant_content_type, video_variant_content_type, ImageProcessor, VariantProcessor,
    VideoProcessor,
};
use tokio::fs;

mod concurrency;
pub(crate) mod processors;
mod subprocess;

/// Meter for everything under `media`, so its metrics group together.
pub(crate) const METER_NAME: &str = "nexus.media";

pub use concurrency::{FailFastGate, MediaGate, MediaPermits, QueuedGate};
/// The only processor type in the public API: `From<MediaProcessorError> for Error` needs it
/// nameable by downstream crates. Everything else under `processors` is an implementation detail.
pub use processors::MediaProcessorError;
pub use subprocess::MediaSubprocess;

#[derive(Clone)]
pub struct VariantController {
    gate: Arc<dyn MediaGate>,
    /// Deadline every subprocess this controller starts runs under.
    subprocess: MediaSubprocess,
}

impl VariantController {
    pub fn new(gate: impl MediaGate + 'static, subprocess: MediaSubprocess) -> Self {
        Self {
            gate: Arc::new(gate),
            subprocess,
        }
    }

    /// The content type a variant is served as. `Main` is the untouched upload, so it keeps the
    /// file's own type; a derived variant carries the type its processor produces, which is why
    /// this dispatches to them rather than restating their formats.
    fn get_content_type_for_variant(file: &FileDetails, variant: &FileVariant) -> String {
        if variant == &FileVariant::Main {
            return file.content_type.clone();
        }
        match &file.content_type {
            content_type if content_type.starts_with("image/") => image_variant_content_type(),
            content_type if content_type.starts_with("video/") => video_variant_content_type(),
            content_type => content_type.clone(),
        }
    }

    /// The content type to serve this variant as, deriving it first if it isn't on disk yet.
    ///
    /// The only way in: deriving a variant has to go through the gate and the deadline this
    /// controller holds, and checking for one that already exists is the cheap path that avoids
    /// spending either.
    pub async fn ensure_variant(
        &self,
        file: &FileDetails,
        variant: &FileVariant,
        file_path: &Path,
    ) -> Result<String, MediaProcessorError> {
        if Self::check_variant_exists(file, *variant, file_path).await {
            return Ok(Self::get_content_type_for_variant(file, variant));
        }

        self.create_file_variant(file, variant, file_path).await
    }

    async fn create_file_variant(
        &self,
        file: &FileDetails,
        variant: &FileVariant,
        file_path: &Path,
    ) -> Result<String, MediaProcessorError> {
        match &file.content_type {
            content_type if content_type.starts_with("image/") => {
                ImageProcessor::create_variant(
                    file,
                    variant,
                    file_path,
                    self.gate.as_ref(),
                    self.subprocess,
                )
                .await
            }
            content_type if content_type.starts_with("video/") => {
                VideoProcessor::create_variant(
                    file,
                    variant,
                    file_path,
                    self.gate.as_ref(),
                    self.subprocess,
                )
                .await
            }
            _ => Err(MediaProcessorError::UnsupportedContentType(
                file.content_type.clone(),
            )),
        }
    }

    async fn check_variant_exists(
        file: &FileDetails,
        variant: FileVariant,
        file_path: &Path,
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
        for content_type in ["video/webm", "video/mp4", "image/png", "application/pdf"] {
            let file = make_file(content_type);
            assert_eq!(
                VariantController::get_content_type_for_variant(&file, &FileVariant::Main),
                content_type
            );
        }
    }

    #[test]
    fn test_derived_image_variants_carry_the_processor_format() {
        let file = make_file("image/png");
        for variant in [FileVariant::Small, FileVariant::Feed] {
            assert_eq!(
                VariantController::get_content_type_for_variant(&file, &variant),
                "image/webp"
            );
        }
    }

    // A content type with no processor keeps its own label; `create_file_variant` is what
    // refuses it, with `UnsupportedContentType`.
    #[test]
    fn test_content_type_without_a_processor_is_passed_through() {
        let file = make_file("application/pdf");
        assert_eq!(
            VariantController::get_content_type_for_variant(&file, &FileVariant::Small),
            "application/pdf"
        );
    }
}

/// Test tooling, shared with the crate's integration tests and benches through the `mock` feature.
#[cfg(any(test, feature = "mock"))]
pub mod test_utils {
    use super::MediaSubprocess;
    use std::time::Duration;

    /// Media subprocess runner for tests: a deadline long enough never to fire on real work.
    pub fn default_subprocess_tests() -> MediaSubprocess {
        MediaSubprocess::new(Duration::from_secs(30))
    }
}
