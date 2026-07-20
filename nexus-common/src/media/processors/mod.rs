use crate::{
    media::{concurrency::MediaGate, FileVariant},
    models::file::FileDetails,
};
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
    #[error("AtCapacity: media processing concurrency limit reached")]
    AtCapacity,
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
    type ProcessingOptions: BaseProcessingOptions + 'static;

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
        gate: &MediaGate,
    ) -> Result<String, MediaProcessorError>
    where
        Self: Sized + 'static,
    {
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

        // Held only around the subprocess, not the path/option work above. The permit moves
        // into the task so it is released when the child exits, not when the caller's future
        // is dropped: a cancelled request must not hand its permit on while its child runs.
        let permit = gate.acquire().await?;
        let content_type = options.content_type();
        let origin_file_path = origin_file_path.to_string();
        let output_path = output_path.to_string();

        tokio::spawn(async move {
            let _permit = permit;
            Self::process(&origin_file_path, &output_path, &options).await
        })
        .await
        .map_err(MediaProcessorError::command_failed)??;

        Ok(content_type)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use crate::media::{concurrency::MediaGate, FileVariant};
    use crate::models::file::{FileDetails, FileUrls};

    use super::{BaseProcessingOptions, MediaProcessorError, VariantProcessor};

    static FINISHED: AtomicBool = AtomicBool::new(false);
    const WORK: Duration = Duration::from_millis(300);

    struct SlowOptions;

    impl BaseProcessingOptions for SlowOptions {
        fn content_type(&self) -> String {
            String::from("image/webp")
        }
    }

    /// Stands in for ImageMagick/ffmpeg: slow, and it records that it ran to completion.
    struct SlowProcessor;

    #[async_trait::async_trait]
    impl VariantProcessor for SlowProcessor {
        type ProcessingOptions = SlowOptions;

        fn get_valid_variants_for_content_type(_content_type: &str) -> Vec<FileVariant> {
            vec![FileVariant::Small]
        }

        fn get_content_type_for_variant(_file: &FileDetails, _variant: &FileVariant) -> String {
            String::from("image/webp")
        }

        fn get_options_for_variant(
            _file: &FileDetails,
            _variant: &FileVariant,
        ) -> Result<SlowOptions, MediaProcessorError> {
            Ok(SlowOptions)
        }

        async fn process(
            _origin_file_path: &str,
            _output_file_path: &str,
            _options: &SlowOptions,
        ) -> Result<String, MediaProcessorError> {
            tokio::time::sleep(WORK).await;
            FINISHED.store(true, Ordering::SeqCst);
            Ok(String::from("image/webp"))
        }
    }

    fn file_details() -> FileDetails {
        FileDetails {
            id: String::from("file"),
            uri: String::new(),
            owner_id: String::from("owner"),
            indexed_at: 0,
            created_at: 0,
            src: String::new(),
            name: String::new(),
            size: 0,
            content_type: String::from("image/png"),
            urls: FileUrls::new(Path::new("/"), &[]),
            metadata: None,
        }
    }

    // A cancelled request must not hand its permit to the next caller while the
    // subprocess is still running, or the gate would undercount live subprocesses.
    #[tokio_shared_rt::test(shared)]
    async fn test_cancelled_request_holds_permit_until_work_completes() {
        let gate = MediaGate::new(1).with_acquire_timeout(Duration::from_millis(50));

        let caller = tokio::spawn({
            let gate = gate.clone();
            async move {
                SlowProcessor::create_variant(
                    &file_details(),
                    &FileVariant::Small,
                    PathBuf::from("/tmp"),
                    &gate,
                )
                .await
            }
        });

        // Let the caller take the only permit, then cancel it mid-work.
        tokio::time::sleep(Duration::from_millis(50)).await;
        caller.abort();
        let _ = caller.await;

        assert!(
            gate.acquire().await.is_err(),
            "permit must stay held while the abandoned subprocess runs"
        );
        assert!(
            !FINISHED.load(Ordering::SeqCst),
            "test must observe the gate before the work completes"
        );

        // Once the work finishes the permit is released and capacity returns.
        tokio::time::sleep(WORK).await;
        assert!(
            FINISHED.load(Ordering::SeqCst),
            "cancelling the caller must not cancel the subprocess"
        );
        assert!(
            gate.acquire().await.is_ok(),
            "permit must be released once the work completes"
        );
    }
}
