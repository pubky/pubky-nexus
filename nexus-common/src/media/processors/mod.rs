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
        matches!(self, Self::AtCapacity | Self::Timeout { .. })
    }
}

/// How long an abandoned temp file must sit before the sweep takes it. Far above any conversion
/// deadline, so a temp file belonging to a run still in flight is never touched.
const TEMP_FILE_MAX_AGE: Duration = Duration::from_secs(60 * 60);

/// A path only this run writes to, so a failure cleans up after itself without touching a variant
/// another request may have just finished.
fn temp_variant_path(output_path: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{output_path}.{}.{nanos}", std::process::id())
}

/// Whether `name` is a temp file for `variant`, i.e. `<variant>.<pid>.<nanos>`.
fn is_temp_variant_of(name: &str, variant: &str) -> bool {
    let Some(suffix) = name.strip_prefix(&format!("{variant}.")) else {
        return false;
    };
    let mut parts = suffix.split('.');
    let (Some(pid), Some(nanos), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !pid.is_empty()
        && !nanos.is_empty()
        && pid.bytes().all(|b| b.is_ascii_digit())
        && nanos.bytes().all(|b| b.is_ascii_digit())
}

/// Drops temp files left behind by a run that never got to clean up, e.g. one killed with the whole
/// process. They are never served -- the variant check stats the exact path -- but they are disk
/// that never comes back.
async fn sweep_stale_temp_files(dir: &Path, variant: &str) {
    let Ok(mut entries) = fs::read_dir(dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_temp_variant_of(name, variant) {
            continue;
        }

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

        // Held only around the subprocess, not the path/option work above. The permit moves
        // into the task so it is released when the child exits, not when the caller's future
        // is dropped: a cancelled request must not hand its permit on while its child runs.
        // TODO: run as a periodic job instead, so conversions stop paying a read_dir each time.
        sweep_stale_temp_files(&origin_path, &variant.to_string()).await;

        // The converter writes here, never to the variant path: an existing variant file *is* the
        // "already made" check, so a killed or crashed child writing there directly would publish
        // its wreckage. Only the rename below makes a variant visible, and only when complete.
        let temp_path = temp_variant_path(output_path);

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

        // Atomic within the directory: a reader sees the finished variant or no variant at all.
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
        is_temp_variant_of, sweep_stale_temp_files, BaseProcessingOptions, MediaProcessorError,
        VariantProcessor, TEMP_FILE_MAX_AGE,
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
        assert!(
            variant_dir_entries(&variant_dir).await.is_empty(),
            "the temp file must be cleaned up too"
        );
    }

    /// Every file sitting in a variant directory, by name.
    async fn variant_dir_entries(dir: &Path) -> Vec<String> {
        let mut entries = tokio::fs::read_dir(dir).await.expect("variant dir");
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await.expect("dir entry") {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        names
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

            // What a concurrent reader would see while the conversion is still running.
            let dir = Path::new(output_file_path)
                .parent()
                .expect("variant dir")
                .to_path_buf();
            let visible = variant_dir_entries(&dir).await;
            OBSERVED.lock().expect("observed").extend(visible);

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
            !during.contains(&variant),
            "the variant path was visible mid-conversion: {during:?}"
        );
        assert!(
            during.iter().any(|name| is_temp_variant_of(name, &variant)),
            "the conversion should write to a temp path: {during:?}"
        );

        // Only the rename publishes it, and it leaves nothing else behind.
        assert_eq!(variant_dir_entries(&dir).await, vec![variant]);
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
            variant_dir_entries(&dir).await.is_empty(),
            "nothing may be published, not even the temp file"
        );
    }

    #[test]
    fn test_temp_variant_name_matching() {
        let cases = [
            ("small.1234.5678", true),
            // The variant itself, and another variant's temp file.
            ("small", false),
            ("feed.1234.5678", false),
            // Shapes that are not <variant>.<pid>.<nanos>.
            ("small.1234", false),
            ("small.1234.5678.9", false),
            ("small.abc.5678", false),
            ("small..5678", false),
        ];

        for (name, expected) in cases {
            assert_eq!(is_temp_variant_of(name, "small"), expected, "{name}");
        }
    }

    // A temp file outlives its run only when the whole process dies, and nothing else will ever
    // clean it up.
    #[tokio_shared_rt::test(shared)]
    async fn test_sweep_removes_only_aged_temp_files() {
        let root = tempfile::TempDir::new().expect("temp dir");
        let file = file_details();
        let dir = variant_dir(root.path(), &file).await;
        let variant = FileVariant::Small.to_string();

        let orphan = dir.join(format!("{variant}.999.111"));
        let in_flight = dir.join(format!("{variant}.888.222"));
        let finished = dir.join(&variant);
        for path in [&orphan, &in_flight, &finished] {
            tokio::fs::write(path, b"x").await.expect("write");
        }

        // Only the orphan is older than the sweep's cutoff.
        let aged = SystemTime::now() - TEMP_FILE_MAX_AGE - Duration::from_secs(60);
        set_modified(&orphan, aged);

        sweep_stale_temp_files(&dir, &variant).await;

        assert_eq!(
            variant_dir_entries(&dir).await,
            vec![variant.clone(), format!("{variant}.888.222")],
            "the sweep must take the orphan and nothing else"
        );
    }

    /// Backdates a file so the sweep sees it as abandoned.
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
