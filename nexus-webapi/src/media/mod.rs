//! Deriving file variants on demand: the concurrency gates that bound it, the subprocess runner
//! it goes through, and the per-format processors that do the converting.
//!
//! Only the API derives variants — a request for one that isn't on disk yet makes it — so the
//! machinery lives here, and so does the content type a derived variant is served under: that
//! label names what a processor writes, so it belongs beside the processor. The variant names and
//! the table of which variants a content type has are shared with the watcher and stay in
//! [`nexus_common::media`].

use std::{path::Path, sync::Arc};

use nexus_common::media::{FileVariant, MediaKind};
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
    /// this dispatches to them rather than restating their formats. A kind without a processor
    /// keeps the file's own type too: there is nothing to derive.
    fn get_content_type_for_variant(file: &FileDetails, variant: &FileVariant) -> String {
        if variant == &FileVariant::Main {
            return file.content_type.clone();
        }
        match MediaKind::from_content_type(&file.content_type) {
            Some(MediaKind::Image) => image_variant_content_type(),
            Some(MediaKind::Video) => video_variant_content_type(),
            None => file.content_type.clone(),
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
        // Keyed on the same `MediaKind` as the variant table in `nexus_common::media`: a content
        // type granted a derived variant there always has a processor here, and one refused
        // here was never granted one.
        match MediaKind::from_content_type(&file.content_type) {
            Some(MediaKind::Image) => {
                ImageProcessor::create_variant(
                    file,
                    variant,
                    file_path,
                    self.gate.as_ref(),
                    self.subprocess,
                )
                .await
            }
            Some(MediaKind::Video) => {
                VideoProcessor::create_variant(
                    file,
                    variant,
                    file_path,
                    self.gate.as_ref(),
                    self.subprocess,
                )
                .await
            }
            None => Err(MediaProcessorError::UnsupportedContentType(
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
    use nexus_common::media::get_valid_variants_for_content_type;

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

    // The stored type is whatever the client sent, and the spec lets a mixed-case one through, so
    // the label must match ignoring case (RFC 2045) or `Image/png` would keep its own type.
    #[test]
    fn test_derived_image_variants_carry_the_processor_format() {
        for content_type in ["image/png", "Image/png"] {
            let file = make_file(content_type);
            for variant in [FileVariant::Small, FileVariant::Feed] {
                assert_eq!(
                    VariantController::get_content_type_for_variant(&file, &variant),
                    "image/webp",
                    "{content_type}"
                );
            }
        }
    }

    // A content type with no processor keeps its own label; `create_file_variant` is what
    // refuses it, with `UnsupportedContentType`. `imagefoo` is one: the kind is the top-level
    // type, not a prefix.
    #[test]
    fn test_content_type_without_a_processor_is_passed_through() {
        for content_type in ["application/pdf", "imagefoo"] {
            let file = make_file(content_type);
            assert_eq!(
                VariantController::get_content_type_for_variant(&file, &FileVariant::Small),
                content_type
            );
        }
    }

    // The dispatcher and the variant table in `nexus_common::media` must agree on every content
    // type the spec lets through, on a type that merely starts with `image`/`video`, and on a
    // mixed-case type (the spec lowercases before validating, but the file keeps the type as
    // sent): a type with no variants at all is exactly the one the dispatcher refuses. Anything
    // granted a variant reaches a processor -- the gate has no permits, so an image sheds with
    // `AtCapacity` before any subprocess starts, and a video answers with its own type because
    // nothing derives yet; neither is `UnsupportedContentType`, which a request would turn into
    // a 500.
    #[tokio_shared_rt::test(shared)]
    async fn test_dispatcher_refuses_exactly_the_content_types_without_variants() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let controller = VariantController::new(
            FailFastGate::new(MediaPermits::new(0)),
            test_utils::default_subprocess_tests(),
        );

        let content_types = pubky_app_specs::VALID_MIME_TYPES.iter().copied().chain([
            "imagefoo",
            "videofoo",
            "Image/png",
            "VIDEO/MP4",
        ]);
        for content_type in content_types {
            let file = make_file(content_type);
            let result = controller
                .create_file_variant(&file, &FileVariant::Small, root.path())
                .await;
            let refused = matches!(result, Err(MediaProcessorError::UnsupportedContentType(_)));
            let has_variants = !get_valid_variants_for_content_type(content_type).is_empty();
            assert_ne!(
                refused, has_variants,
                "{content_type}: variants {has_variants}, dispatcher answered {result:?}"
            );
        }
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
