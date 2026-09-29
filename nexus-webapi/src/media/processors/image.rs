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
        let origin = ImageProcessor::identify(origin_file_path, subprocess).await?;

        let output = match origin.format == options.format {
            true => output_file_path.to_string(),
            false => format!("{}:{}", options.format, output_file_path),
        };

        let mut command = Command::new("convert");
        command.arg(origin_file_path);
        // Frame-optimized animations store partial sub-frames; flatten each onto the full canvas
        // so resize scales complete frames instead of fragments. Only for animations: coalescing
        // a still image composes it onto its stored page canvas, so a cropped PNG that kept its
        // page offset would come out padded to the page size instead of its visible size.
        if origin.frames > 1 {
            command.arg("-coalesce");
        }
        command
            .arg("-resize")
            .arg(resize_geometry(&options.width))
            .arg("-auto-orient") // https://github.com/ImageMagick/ImageMagick/issues/6396
            .arg(output);

        let child_output = subprocess.run(&mut command).await?;

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

/// What `identify` reports about a source image.
#[derive(Debug, PartialEq)]
struct SourceImage {
    /// The ImageMagick format name, lowercased (e.g. `gif`, `webp`).
    format: String,
    /// How many frames the image stores: 1 for a still image, more for an animation.
    frames: u32,
}

impl SourceImage {
    /// Parses `identify -format "%m %n\n"` output. `identify` prints the line once per frame, so
    /// only the first is read.
    fn parse(listing: &str) -> Option<Self> {
        let (format, frames) = listing.lines().next()?.trim().split_once(' ')?;
        Some(SourceImage {
            format: format.to_lowercase(),
            frames: frames.parse().ok()?,
        })
    }
}

impl ImageProcessor {
    async fn identify(
        file_path: &str,
        subprocess: MediaSubprocess,
    ) -> Result<SourceImage, MediaProcessorError> {
        let child_output = subprocess
            .run(
                Command::new("identify")
                    .arg("-format")
                    .arg("%m %n\\n")
                    .arg(file_path),
            )
            .await?;

        if !child_output.status.success() {
            return Err(MediaProcessorError::command_failed(format!(
                "ImageMagick format extraction failed: {}",
                String::from_utf8_lossy(&child_output.stderr)
            )));
        }

        let listing = String::from_utf8_lossy(&child_output.stdout);
        SourceImage::parse(&listing).ok_or_else(|| {
            MediaProcessorError::command_failed(format!(
                "ImageMagick format extraction returned unexpected output: {listing:?}"
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use super::*;

    // The label and the bytes are one fact. If `IMAGE_FORMAT` changes, this fails and whoever
    // changed it has to confirm the served content type is meant to change with it.
    #[test]
    fn test_variant_content_type_tracks_the_output_format() {
        assert_eq!(IMAGE_FORMAT, "webp");
        assert_eq!(image_variant_content_type(), "image/webp");
    }

    #[test]
    fn test_resize_geometry_never_enlarges() {
        assert_eq!(resize_geometry("320"), "320x>");
        assert_eq!(resize_geometry("720"), "720x>");
        assert_eq!(resize_geometry("1440"), "1440x>");
    }

    // One assertion per variant, so a width cannot change, or a variant appear, unlisted.
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

    #[test]
    fn test_source_image_reads_format_and_frame_count_from_the_first_frame() {
        // `identify` repeats the line for every frame of an animation.
        assert_eq!(
            SourceImage::parse("GIF 3\nGIF 3\nGIF 3\n"),
            Some(SourceImage {
                format: "gif".into(),
                frames: 3
            })
        );
        assert_eq!(
            SourceImage::parse("PNG 1\n"),
            Some(SourceImage {
                format: "png".into(),
                frames: 1
            })
        );
        assert_eq!(SourceImage::parse(""), None);
        assert_eq!(SourceImage::parse("PNG\n"), None);
        assert_eq!(SourceImage::parse("PNG x\n"), None);
    }

    /// Runs an ImageMagick command through the media runner and returns its stdout, failing the
    /// test if the command does not exit cleanly.
    async fn magick(runner: MediaSubprocess, command: &mut Command) -> String {
        let output = runner
            .run(command)
            .await
            .unwrap_or_else(|error| panic!("{command:?} did not run: {error}"));
        assert!(
            output.status.success(),
            "{command:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Writes a three-frame animated GIF: a red 400x400 canvas with a blue square that moves
    /// diagonally between frames. `-layers Optimize` stores frames 1 and 2 as sub-frames covering
    /// only the region that changed, which is the shape of input the `-coalesce` fix exists for.
    async fn write_frame_optimized_gif(runner: MediaSubprocess, path: &Path) {
        let mut command = Command::new("convert");
        command
            .args(["-size", "400x400", "xc:red", "-fill", "blue"])
            .args(["-draw", "rectangle 0,0 50,50"])
            .args(["(", "-size", "400x400", "xc:red", "-fill", "blue"])
            .args(["-draw", "rectangle 100,100 150,150", ")"])
            .args(["(", "-size", "400x400", "xc:red", "-fill", "blue"])
            .args(["-draw", "rectangle 200,200 250,250", ")"])
            .args(["-set", "delay", "20", "-loop", "0", "-layers", "Optimize"])
            .arg(path);
        magick(runner, &mut command).await;
    }

    /// The `(width, height)` of every frame stored in `path`, in frame order.
    async fn frame_sizes(runner: MediaSubprocess, path: &Path) -> Vec<(u32, u32)> {
        let listing = magick(
            runner,
            Command::new("identify")
                .args(["-format", "%w %h\\n"])
                .arg(path),
        )
        .await;
        listing
            .lines()
            .map(|line| {
                let (width, height) = line
                    .split_once(' ')
                    .unwrap_or_else(|| panic!("unexpected identify line {line:?}"));
                (
                    width.parse().expect("frame width"),
                    height.parse().expect("frame height"),
                )
            })
            .collect()
    }

    /// Splits `path` into one fully composed PNG per frame under `dir`, returning them in frame
    /// order. Coalescing here matters: the WebP encoder re-optimizes frames, so sampling the
    /// stored frames directly would read sub-frames again.
    async fn coalesced_frames(runner: MediaSubprocess, path: &Path, dir: &Path) -> Vec<PathBuf> {
        magick(
            runner,
            Command::new("convert")
                .arg(path)
                .arg("-coalesce")
                .arg(dir.join("frame-%d.png")),
        )
        .await;

        let mut frames: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("read the frames dir")
            .map(|entry| entry.expect("dir entry").path())
            .filter(|entry| {
                entry
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("frame-") && name.ends_with(".png"))
            })
            .collect();
        // `frame-%d` is not zero-padded, so sort by the frame index rather than the name.
        frames.sort_by_key(|frame| frame_index(frame));
        frames
    }

    fn frame_index(frame: &Path) -> u32 {
        frame
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.strip_prefix("frame-"))
            .and_then(|index| index.parse().ok())
            .unwrap_or_else(|| panic!("frame path {} has no index", frame.display()))
    }

    /// The `(r, g, b)` of the pixel at `(x, y)` in `frame`, on a 0..=255 scale.
    async fn pixel_at(runner: MediaSubprocess, frame: &Path, x: u32, y: u32) -> (u32, u32, u32) {
        let pixel = magick(
            runner,
            Command::new("convert")
                .arg(frame)
                .arg("-format")
                .arg(format!("%[pixel:p{{{x},{y}}}]"))
                .arg("info:"),
        )
        .await;
        parse_rgb(pixel.trim())
    }

    /// Parses the `%[pixel:...]` output. ImageMagick prints an `srgb(r,g,b)` or `srgba(r,g,b,a)`
    /// tuple for 8-bit images; older versions print a colour name when the value is an exact match,
    /// and `red` is the only name this test can meet.
    fn parse_rgb(pixel: &str) -> (u32, u32, u32) {
        if pixel == "red" {
            return (255, 0, 0);
        }
        let channels: Vec<u32> = pixel
            .strip_prefix("srgba(")
            .or_else(|| pixel.strip_prefix("srgb("))
            .and_then(|rest| rest.strip_suffix(')'))
            .unwrap_or_else(|| panic!("unexpected pixel value {pixel:?}"))
            .split(',')
            .take(3)
            .map(|channel| {
                channel
                    .trim()
                    .parse()
                    .unwrap_or_else(|_| panic!("unexpected pixel value {pixel:?}"))
            })
            .collect();
        match channels[..] {
            [r, g, b] => (r, g, b),
            _ => panic!("unexpected pixel value {pixel:?}"),
        }
    }

    // Regression test for the `-coalesce` before `-resize` in `process`. A frame-optimized GIF
    // stores later frames as sub-frames covering only what changed. Resizing those fragments on
    // their own scales each to the target width as if it were the whole picture, so the resized
    // animation loses its background (and, on ImageMagick 6.9.12, one frame outright).
    #[tokio_shared_rt::test(shared)]
    async fn test_process_keeps_every_frame_of_a_frame_optimized_gif_on_its_background() {
        let runner = MediaSubprocess::new(Duration::from_secs(30));
        let dir = tempfile::TempDir::new().expect("temp dir");
        let origin = dir.path().join("anim.gif");
        let output = dir.path().join("small.webp");

        write_frame_optimized_gif(runner, &origin).await;

        // The fixture must really be frame-optimized, or the test would pass without the fix.
        let input_sizes = frame_sizes(runner, &origin).await;
        assert_eq!(input_sizes.len(), 3, "the fixture must have three frames");
        for (index, (width, height)) in input_sizes.iter().enumerate().skip(1) {
            assert!(
                *width < 400 && *height < 400,
                "fixture frame {index} is {width}x{height}: it must be a sub-frame of the 400x400 canvas"
            );
        }

        let options = ImageProcessor::get_options_for_variant(&FileVariant::Small)
            .expect("Small is a supported image variant");
        ImageProcessor::process(
            origin.to_str().expect("utf-8 origin path"),
            output.to_str().expect("utf-8 output path"),
            &options,
            runner,
        )
        .await
        .expect("the resize must succeed");

        let frames_dir = dir.path().join("frames");
        std::fs::create_dir(&frames_dir).expect("create the frames dir");
        let frames = coalesced_frames(runner, &output, &frames_dir).await;
        assert_eq!(
            frames.len(),
            input_sizes.len(),
            "the resized animation must keep every input frame"
        );

        // (5, 78) at 320px is background in every frame: left of the frame-0 square's column and
        // below its row, and outside the later squares entirely. Red survives WebP's lossy
        // encoding with a little noise, hence thresholds rather than exact channel values.
        for frame in &frames {
            let (r, _, b) = pixel_at(runner, frame, 5, 78).await;
            assert!(
                r > 200 && b < 50,
                "{} has background pixel srgb({r},_,{b}) at (5,78): expected red, so the frame was resized off its canvas",
                frame.display()
            );
        }
    }

    /// Writes a 640x320 red PNG that keeps a page geometry of `page`, the shape `-crop` leaves
    /// behind when it is not followed by `+repage`. Browsers ignore the stored page and show the
    /// 640x320 image as it is.
    async fn write_paged_png(runner: MediaSubprocess, path: &Path, page: &str) {
        let mut command = Command::new("convert");
        command
            .args(["-size", "640x320", "xc:red", "-page", page])
            .arg(path);
        magick(runner, &mut command).await;
    }

    // Regression test for gating `-coalesce` on the frame count. Coalescing a still image composes
    // it onto its stored page canvas: a positive offset pads the output out to the page size, and
    // a negative one clips visible pixels off. A still image must resize as the pixels it holds.
    #[tokio_shared_rt::test(shared)]
    async fn test_process_resizes_a_still_image_by_its_pixels_not_its_page_canvas() {
        let runner = MediaSubprocess::new(Duration::from_secs(30));
        let options = ImageProcessor::get_options_for_variant(&FileVariant::Small)
            .expect("Small is a supported image variant");

        // `1600x1600+320+480` is what `-crop 640x320+320+480` leaves on a 1600x1600 image. The
        // image is wider than the Small width so the resize has to shrink it.
        for page in ["1600x1600+320+480", "1600x1600-160-160"] {
            let dir = tempfile::TempDir::new().expect("temp dir");
            let origin = dir.path().join("paged.png");
            let output = dir.path().join("small.webp");

            write_paged_png(runner, &origin, page).await;
            assert_eq!(
                frame_sizes(runner, &origin).await,
                vec![(640, 320)],
                "the {page} fixture must be a single 640x320 frame"
            );

            ImageProcessor::process(
                origin.to_str().expect("utf-8 origin path"),
                output.to_str().expect("utf-8 output path"),
                &options,
                runner,
            )
            .await
            .expect("the resize must succeed");

            assert_eq!(
                frame_sizes(runner, &output).await,
                vec![(320, 160)],
                "a 640x320 image on page {page} must resize to 320x160"
            );

            // Every corner must still be the image's red, not canvas padding or a clipped edge.
            for (x, y) in [(2, 2), (317, 2), (2, 157), (317, 157)] {
                let (r, g, b) = pixel_at(runner, &output, x, y).await;
                assert!(
                    r > 200 && g < 50 && b < 50,
                    "page {page}: pixel ({x},{y}) is srgb({r},{g},{b}), expected red"
                );
            }
        }
    }
}
