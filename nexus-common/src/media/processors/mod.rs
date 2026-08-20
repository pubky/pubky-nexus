use crate::{
    media::{concurrency::MediaGate, FileVariant, MediaSubprocess},
    models::file::FileDetails,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::fs;

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
    #[error("Timeout: {command} exceeded {deadline:?}")]
    Timeout { command: String, deadline: Duration },
    #[error("SkippedAfterTimeout: converting {file} was killed recently; not retrying yet")]
    SkippedAfterTimeout { file: String },
}

impl MediaProcessorError {
    pub fn command_failed(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::CommandFailed {
            source: source.into(),
        }
    }

    /// Whether this means "no variant right now" rather than "this file is broken", so the caller
    /// degrades (serves `main`, answers 503) instead of reporting a server fault.
    pub fn is_load_shed(&self) -> bool {
        matches!(
            self,
            Self::AtCapacity | Self::Timeout { .. } | Self::SkippedAfterTimeout { .. }
        )
    }
}

/// How long an abandoned temp file must sit before the sweep takes it. Far above any conversion
/// deadline, so a temp file belonging to a run still in flight is never touched.
const TEMP_FILE_MAX_AGE: Duration = Duration::from_secs(60 * 60);

/// Conversions write here rather than beside the variants, so the strays left by a process that
/// died mid-conversion are one directory to list instead of a walk over every file on disk. Inside
/// `files_path` on purpose: `rename` is atomic within a filesystem, and nesting it here makes that
/// structural rather than a deployment assumption. No request can address it -- the owner segment
/// of a served path is always a `PubkyId`.
const TEMP_DIR_NAME: &str = ".tmp";

/// How long a killed conversion is remembered. Far above any deadline, so a hostile file costs one
/// conversion per hour rather than one per request, while a file that timed out because the box was
/// loaded is retried within the hour.
const TIMEDOUT_MARKER_TTL: Duration = Duration::from_secs(60 * 60);

/// Records that converting this file was killed, so the next request sheds instead of spending
/// another full deadline learning the same thing. Per file rather than per variant: every variant
/// decodes the same source, which is where a hostile file burns the time, so `small` timing out
/// tells us what `feed` would find. It lives beside the variants, not in the temp directory: it is
/// state tied to the file, and deleting the file takes it along. Named so it cannot collide with a
/// variant, which is always one of `FileVariant`.
const TIMEDOUT_MARKER_NAME: &str = ".timedout";

/// Whether a killed conversion is still recent enough to skip retrying. Clears the marker once it
/// expires -- this is the only place that stats it, so reclaiming it here costs nothing.
async fn timed_out_recently(marker_path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(marker_path).await else {
        return false;
    };

    let recent = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age < TIMEDOUT_MARKER_TTL);

    if !recent {
        let _ = fs::remove_file(marker_path).await;
    }
    recent
}

/// A name only this run writes to, so a failure cleans up after itself without touching a variant
/// another request may have just finished. It carries the file it belongs to so an orphan in the
/// temp directory can be traced back.
fn temp_variant_name(file: &FileDetails, variant: &FileVariant) -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{}.{}.{variant}.{}.{nanos}",
        file.owner_id,
        file.id,
        std::process::id()
    )
}

/// Drops temp files left behind by a run that never got to clean up, e.g. one killed with the whole
/// process. Everything in the temp directory is one of ours, so age is the only question.
async fn sweep_stale_temp_files(temp_dir: &Path) {
    let Ok(mut entries) = fs::read_dir(temp_dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let aged = match entry.metadata().await.and_then(|m| m.modified()) {
            Ok(modified) => modified.elapsed().is_ok_and(|age| age > TEMP_FILE_MAX_AGE),
            Err(_) => false,
        };
        if aged {
            let _ = fs::remove_file(entry.path()).await;
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
        subprocess: MediaSubprocess,
    ) -> Result<String, MediaProcessorError>;

    /// Creates a variant for the given file
    /// If there are no options for this variant, return with the original content type
    async fn create_variant(
        file: &FileDetails,
        variant: &FileVariant,
        file_path: PathBuf,
        gate: &dyn MediaGate,
        subprocess: MediaSubprocess,
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

        // A killed conversion will be killed again, so shed here rather than spend another full
        // deadline on it. Checked before the permit: a retry we already know the answer to must not
        // occupy a slot other files could use.
        let marker_path = origin_path.join(TIMEDOUT_MARKER_NAME);
        if timed_out_recently(&marker_path).await {
            tracing::debug!(
                "Skipping {variant} for file {}: converting it was killed within the last hour",
                file.id
            );
            return Err(MediaProcessorError::SkippedAfterTimeout {
                file: file.id.clone(),
            });
        }

        // The converter writes here, never to the variant path: an existing variant file *is* the
        // "already made" check, so a killed or crashed child writing there directly would publish
        // its wreckage. Only the rename below makes a variant visible, and only when complete.
        let temp_dir = file_path.join(TEMP_DIR_NAME);
        fs::create_dir_all(&temp_dir)
            .await
            .map_err(MediaProcessorError::command_failed)?;

        // TODO: run as a periodic job instead, so conversions stop paying a read_dir each time.
        sweep_stale_temp_files(&temp_dir).await;

        let temp_file = temp_dir.join(temp_variant_name(file, variant));
        let Some(temp_path) = temp_file.to_str() else {
            return Err(MediaProcessorError::InvalidFilePath(
                "Temp file".to_string(),
            ));
        };
        let temp_path = temp_path.to_string();

        // Held only around the subprocess, not the path/option work above. The permit moves
        // into the task so it is released when the child exits, not when the caller's future
        // is dropped: a cancelled request must not hand its permit on while its child runs.
        let permit = gate.acquire().await?;
        let content_type = options.content_type();
        let origin_file_path = origin_file_path.to_string();
        let output_path = output_path.to_string();

        let processed = tokio::spawn({
            let temp_path = temp_path.clone();
            async move {
                let _permit = permit;
                Self::process(&origin_file_path, &temp_path, &options, subprocess).await
            }
        })
        .await
        .map_err(MediaProcessorError::command_failed)?;

        if let Err(error) = processed {
            // Only ever this run's own file, so a concurrent run that just finished keeps its work.
            let _ = fs::remove_file(&temp_path).await;

            if matches!(error, MediaProcessorError::Timeout { .. }) {
                // A killed child means a tool-level limit was escaped, so it is worth its own line:
                // the shed answer the caller returns names neither the file nor the command.
                tracing::warn!("Media subprocess killed for file {}: {error}", file.id);
                // Remembered so the next request sheds instead of repeating the same deadline.
                let _ = fs::write(&marker_path, b"").await;
            }
            return Err(error);
        }

        // A killed converter can exit having written nothing; publishing that would cache an empty
        // variant forever.
        let written = fs::metadata(&temp_path)
            .await
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        if written == 0 {
            let _ = fs::remove_file(&temp_path).await;
            return Err(MediaProcessorError::command_failed(format!(
                "conversion produced no output for file {}",
                file.id
            )));
        }

        // Atomic: both paths are under `files_path`, so this is a same-filesystem rename and a
        // reader sees the finished variant or no variant at all.
        fs::rename(&temp_path, &output_path)
            .await
            .map_err(MediaProcessorError::command_failed)?;

        Ok(content_type)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime};

    use crate::media::{
        concurrency::{MediaPermits, QueuedGate},
        FileVariant, MediaGate, MediaSubprocess,
    };
    use crate::models::file::{FileDetails, FileUrls};

    use super::{
        sweep_stale_temp_files, timed_out_recently, BaseProcessingOptions, MediaProcessorError,
        VariantProcessor, TEMP_DIR_NAME, TEMP_FILE_MAX_AGE, TIMEDOUT_MARKER_NAME,
        TIMEDOUT_MARKER_TTL,
    };

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
            _subprocess: MediaSubprocess,
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
        let gate = Arc::new(QueuedGate::with_limits(
            MediaPermits::new(1),
            4,
            Duration::from_millis(50),
        ));

        let caller = tokio::spawn({
            let gate = Arc::clone(&gate);
            async move {
                SlowProcessor::create_variant(
                    &file_details(),
                    &FileVariant::Small,
                    PathBuf::from("/tmp"),
                    gate.as_ref(),
                    MediaSubprocess::new(Duration::from_secs(30)),
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

    /// Writes the half-finished output a killed converter would leave behind, then fails.
    struct HalfWrittenProcessor;

    #[async_trait::async_trait]
    impl VariantProcessor for HalfWrittenProcessor {
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
            output_file_path: &str,
            _options: &SlowOptions,
            _subprocess: MediaSubprocess,
        ) -> Result<String, MediaProcessorError> {
            tokio::fs::write(output_file_path, b"")
                .await
                .expect("the stand-in must leave its wreckage behind");
            Err(MediaProcessorError::Timeout {
                command: String::from("convert"),
                deadline: Duration::from_millis(1),
            })
        }
    }

    // An existing file is the "variant already made" check, so a half-written one would be served
    // as the variant from then on.
    #[tokio_shared_rt::test(shared)]
    async fn test_failed_conversion_leaves_no_output_behind() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let variant_dir = variant_dir(root.path(), &file).await;
        let output = variant_dir.join(FileVariant::Small.to_string());

        let gate = QueuedGate::with_limits(MediaPermits::new(1), 4, Duration::from_millis(200));
        let result = HalfWrittenProcessor::create_variant(
            &file,
            &FileVariant::Small,
            root.path().to_path_buf(),
            &gate,
            MediaSubprocess::new(Duration::from_secs(30)),
        )
        .await;

        assert!(result.is_err(), "the conversion failed");
        assert!(
            !output.exists(),
            "a failed conversion must not leave a variant behind"
        );
        assert_eq!(
            dir_entries(&variant_dir).await,
            vec![TIMEDOUT_MARKER_NAME.to_string()],
            "only the marker remains: no variant, no temp file"
        );
    }

    /// Every file sitting in a directory, by name.
    async fn dir_entries(dir: &Path) -> Vec<String> {
        let mut entries = tokio::fs::read_dir(dir).await.expect("variant dir");
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await.expect("dir entry") {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        names
    }

    /// Every file in the shared temp directory, by name.
    async fn temp_dir_entries(root: &Path) -> Vec<String> {
        let dir = root.join(TEMP_DIR_NAME);
        if !dir.exists() {
            return Vec::new();
        }
        dir_entries(&dir).await
    }

    /// Creates the variant directory a processor writes into, and returns it.
    async fn variant_dir(root: &Path, file: &FileDetails) -> PathBuf {
        let dir = root.join(file.owner_id.as_str()).join(file.id.as_str());
        tokio::fs::create_dir_all(&dir).await.expect("variant dir");
        dir
    }

    /// Reports the variant path it was handed, so a test can watch what is visible mid-conversion.
    struct ReportingProcessor;

    static OBSERVED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    /// Directory the stand-in inspects mid-conversion.
    static WATCHED: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

    #[async_trait::async_trait]
    impl VariantProcessor for ReportingProcessor {
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
            output_file_path: &str,
            _options: &SlowOptions,
            _subprocess: MediaSubprocess,
        ) -> Result<String, MediaProcessorError> {
            tokio::fs::write(output_file_path, b"variant bytes")
                .await
                .expect("stand-in must write its output");

            // What a concurrent reader would see in the variant directory while this runs.
            let watched = WATCHED.lock().expect("watched").clone();
            if let Some(dir) = watched {
                let visible = dir_entries(&dir).await;
                OBSERVED.lock().expect("observed").extend(visible);
            }

            Ok(String::from("image/webp"))
        }
    }

    // The variant path must stay absent until the conversion is complete, or a concurrent request
    // stats it mid-write and serves a truncated file.
    #[tokio_shared_rt::test(shared)]
    async fn test_variant_is_invisible_until_complete() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let dir = variant_dir(root.path(), &file).await;
        let variant = FileVariant::Small.to_string();
        OBSERVED.lock().expect("observed").clear();
        *WATCHED.lock().expect("watched") = Some(dir.clone());

        let gate = QueuedGate::with_limits(MediaPermits::new(1), 4, Duration::from_millis(200));
        ReportingProcessor::create_variant(
            &file,
            &FileVariant::Small,
            root.path().to_path_buf(),
            &gate,
            MediaSubprocess::new(Duration::from_secs(30)),
        )
        .await
        .expect("conversion succeeds");

        let during = OBSERVED.lock().expect("observed").clone();
        assert!(
            during.is_empty(),
            "nothing may be visible beside the variants mid-conversion: {during:?}"
        );

        // Only the rename publishes it, and it leaves nothing else behind.
        assert_eq!(dir_entries(&dir).await, vec![variant]);
        assert!(
            temp_dir_entries(root.path()).await.is_empty(),
            "the temp file is consumed by the rename"
        );
    }

    // A run that fails must not delete a variant another request just finished.
    #[tokio_shared_rt::test(shared)]
    async fn test_failed_conversion_keeps_a_finished_variant() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let dir = variant_dir(root.path(), &file).await;
        let output = dir.join(FileVariant::Small.to_string());
        tokio::fs::write(&output, b"a variant someone else finished")
            .await
            .expect("existing variant");

        let gate = QueuedGate::with_limits(MediaPermits::new(1), 4, Duration::from_millis(200));
        let result = HalfWrittenProcessor::create_variant(
            &file,
            &FileVariant::Small,
            root.path().to_path_buf(),
            &gate,
            MediaSubprocess::new(Duration::from_secs(30)),
        )
        .await;

        assert!(result.is_err(), "this conversion failed");
        assert_eq!(
            tokio::fs::read(&output).await.expect("variant still there"),
            b"a variant someone else finished",
            "a failing run must not touch a variant it did not write"
        );
    }

    // An empty output is what a killed converter leaves; publishing it would cache nothing forever.
    #[tokio_shared_rt::test(shared)]
    async fn test_empty_output_is_not_published() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let dir = variant_dir(root.path(), &file).await;
        let gate = QueuedGate::with_limits(MediaPermits::new(1), 4, Duration::from_millis(200));

        let result = EmptyOutputProcessor::create_variant(
            &file,
            &FileVariant::Small,
            root.path().to_path_buf(),
            &gate,
            MediaSubprocess::new(Duration::from_secs(30)),
        )
        .await;

        assert!(result.is_err(), "an empty conversion is a failure");
        assert!(
            dir_entries(&dir).await.is_empty(),
            "nothing may be published, not even the temp file"
        );
    }

    /// Records whether it ran, so a test can prove a shed never reached the converter.
    struct RecordingProcessor;

    #[async_trait::async_trait]
    impl VariantProcessor for RecordingProcessor {
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
            output_file_path: &str,
            _options: &SlowOptions,
            _subprocess: MediaSubprocess,
        ) -> Result<String, MediaProcessorError> {
            tokio::fs::write(output_file_path, b"variant bytes")
                .await
                .expect("stand-in must write its output");
            Ok(String::from("image/webp"))
        }
    }

    /// The marker a killed conversion leaves for the file, whichever variant was being made.
    fn marker_path(dir: &Path) -> PathBuf {
        dir.join(TIMEDOUT_MARKER_NAME)
    }

    async fn create_small<P: VariantProcessor + Send + Sync + Sized + 'static>(
        root: &Path,
        file: &FileDetails,
        gate: &dyn MediaGate,
    ) -> Result<String, MediaProcessorError> {
        P::create_variant(
            file,
            &FileVariant::Small,
            root.to_path_buf(),
            gate,
            MediaSubprocess::new(Duration::from_secs(30)),
        )
        .await
    }

    // A conversion we already killed will be killed again, so it must not cost a second deadline --
    // or a permit, which is what a hostile file would use to keep the gate full.
    #[tokio_shared_rt::test(shared)]
    async fn test_recent_timeout_sheds_without_converting() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let dir = variant_dir(root.path(), &file).await;
        tokio::fs::write(marker_path(&dir), b"")
            .await
            .expect("marker");

        let gate = QueuedGate::with_limits(MediaPermits::new(1), 4, Duration::from_millis(200));
        let result = create_small::<RecordingProcessor>(root.path(), &file, &gate).await;

        assert!(
            matches!(result, Err(MediaProcessorError::SkippedAfterTimeout { .. })),
            "expected a shed, got {result:?}"
        );
        assert!(
            !dir.join(FileVariant::Small.to_string()).exists()
                && temp_dir_entries(root.path()).await.is_empty(),
            "the converter must not run"
        );
        assert!(
            gate.acquire().await.is_ok(),
            "a shed must not consume a permit"
        );
    }

    // Every variant decodes the same source, so one kill answers for all of them: `feed` must not
    // spend its own deadline rediscovering what `small` already found.
    #[tokio_shared_rt::test(shared)]
    async fn test_a_kill_covers_every_variant_of_the_file() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let dir = variant_dir(root.path(), &file).await;
        let gate = QueuedGate::with_limits(MediaPermits::new(1), 4, Duration::from_millis(200));

        // `small` is killed.
        let killed = create_small::<HalfWrittenProcessor>(root.path(), &file, &gate).await;
        assert!(matches!(killed, Err(MediaProcessorError::Timeout { .. })));
        assert!(marker_path(&dir).exists());

        // `feed` then sheds on that marker rather than converting.
        let feed = RecordingProcessor::create_variant(
            &file,
            &FileVariant::Feed,
            root.path().to_path_buf(),
            &gate,
            MediaSubprocess::new(Duration::from_secs(30)),
        )
        .await;

        assert!(
            matches!(feed, Err(MediaProcessorError::SkippedAfterTimeout { .. })),
            "expected a shed, got {feed:?}"
        );
        assert!(
            !dir.join(FileVariant::Feed.to_string()).exists(),
            "the converter must not run"
        );
    }

    // Past the TTL the file gets another chance, so a timeout caused by a loaded box heals.
    #[tokio_shared_rt::test(shared)]
    async fn test_expired_marker_lets_the_conversion_run() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let dir = variant_dir(root.path(), &file).await;
        let marker = marker_path(&dir);
        tokio::fs::write(&marker, b"").await.expect("marker");
        set_modified(
            &marker,
            SystemTime::now() - TIMEDOUT_MARKER_TTL - Duration::from_secs(60),
        );

        let gate = QueuedGate::with_limits(MediaPermits::new(1), 4, Duration::from_millis(200));
        create_small::<RecordingProcessor>(root.path(), &file, &gate)
            .await
            .expect("an expired marker must not block the conversion");

        assert!(
            dir.join(FileVariant::Small.to_string()).exists(),
            "the converter must run"
        );
        assert!(!marker.exists(), "the expiry check reclaims the marker");
    }

    // Only killed conversions are worth remembering: an ordinary failure is cheap to retry.
    #[tokio_shared_rt::test(shared)]
    async fn test_only_a_timeout_is_remembered() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let dir = variant_dir(root.path(), &file).await;

        let gate = QueuedGate::with_limits(MediaPermits::new(1), 4, Duration::from_millis(200));

        // HalfWrittenProcessor fails with a timeout.
        let timed_out = create_small::<HalfWrittenProcessor>(root.path(), &file, &gate).await;
        assert!(timed_out.is_err());
        assert!(
            marker_path(&dir).exists(),
            "a killed conversion must be remembered"
        );
        tokio::fs::remove_file(marker_path(&dir))
            .await
            .expect("clear for the next case");

        // EmptyOutputProcessor fails without one.
        let failed = create_small::<EmptyOutputProcessor>(root.path(), &file, &gate).await;
        assert!(failed.is_err());
        assert!(
            !marker_path(&dir).exists(),
            "an ordinary failure stays retryable"
        );
    }

    // A temp file outlives its run only when the whole process dies, and nothing else will ever
    // clean it up.
    #[tokio_shared_rt::test(shared)]
    async fn test_sweep_removes_only_aged_temp_files() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let temp_dir = root.path().join(TEMP_DIR_NAME);
        tokio::fs::create_dir_all(&temp_dir)
            .await
            .expect("temp dir");

        let orphan = temp_dir.join("owner.file.small.999.111");
        let in_flight = temp_dir.join("owner.file.feed.888.222");
        for path in [&orphan, &in_flight] {
            tokio::fs::write(path, b"x").await.expect("write");
        }

        // Only the orphan is older than the sweep's cutoff.
        set_modified(
            &orphan,
            SystemTime::now() - TEMP_FILE_MAX_AGE - Duration::from_secs(60),
        );

        sweep_stale_temp_files(&temp_dir).await;

        assert_eq!(
            dir_entries(&temp_dir).await,
            vec![String::from("owner.file.feed.888.222")],
            "the sweep must take the orphan and nothing else"
        );
    }

    // Nothing walks the variant directories any more, so the expiry check is what reclaims a
    // marker -- it already stats it, so this costs nothing.
    #[tokio_shared_rt::test(shared)]
    async fn test_expired_marker_is_reclaimed_by_the_check() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let dir = variant_dir(root.path(), &file).await;
        let marker = marker_path(&dir);
        tokio::fs::write(&marker, b"").await.expect("marker");

        assert!(timed_out_recently(&marker).await, "a fresh marker sheds");
        assert!(marker.exists(), "and survives the check");

        set_modified(
            &marker,
            SystemTime::now() - TIMEDOUT_MARKER_TTL - Duration::from_secs(60),
        );
        assert!(
            !timed_out_recently(&marker).await,
            "an expired marker does not"
        );
        assert!(!marker.exists(), "and is reclaimed on the way out");
    }

    /// Backdates a file so it reads as abandoned.
    fn set_modified(path: &Path, when: SystemTime) {
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open for backdating");
        file.set_modified(when).expect("backdate");
    }

    /// Exits successfully having written an empty file, as a killed converter does.
    struct EmptyOutputProcessor;

    #[async_trait::async_trait]
    impl VariantProcessor for EmptyOutputProcessor {
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
            output_file_path: &str,
            _options: &SlowOptions,
            _subprocess: MediaSubprocess,
        ) -> Result<String, MediaProcessorError> {
            tokio::fs::write(output_file_path, b"")
                .await
                .expect("stand-in must write an empty output");
            Ok(String::from("image/webp"))
        }
    }

    /// Runs a child that never exits on its own, so only the deadline can end it.
    struct WedgedProcessor;

    #[async_trait::async_trait]
    impl VariantProcessor for WedgedProcessor {
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
            subprocess: MediaSubprocess,
        ) -> Result<String, MediaProcessorError> {
            subprocess
                .run(tokio::process::Command::new("sleep").arg("30"))
                .await?;
            Ok(String::from("image/webp"))
        }
    }

    // Without a deadline a hung subprocess keeps its permit until restart, and enough of them
    // exhaust the gate.
    #[tokio_shared_rt::test(shared)]
    async fn test_wedged_subprocess_releases_its_permit_at_the_deadline() {
        let gate = QueuedGate::with_limits(
            MediaPermits::new(1),
            4,
            // Short enough that the assertion below fails fast if the permit is never returned.
            Duration::from_millis(200),
        );

        let result = WedgedProcessor::create_variant(
            &file_details(),
            &FileVariant::Small,
            PathBuf::from("/tmp"),
            &gate,
            MediaSubprocess::new(Duration::from_millis(100)),
        )
        .await;

        assert!(
            matches!(result, Err(MediaProcessorError::Timeout { .. })),
            "the deadline must surface as a timeout, got {result:?}"
        );
        assert!(
            gate.acquire().await.is_ok(),
            "killing the child must return its permit to the pool"
        );
    }
}
