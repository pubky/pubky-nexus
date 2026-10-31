//! `/v1/static/files/{owner_id}/{file_id}/{variant}`: the one base path a v1 client reads file
//! bytes from. A `{tsid}` is served from its `File` node and legacy folder, as `/static/files`
//! serves it; a `{hash}.{ext}` from the owner's v1 folder, with no node. Driven through the
//! router with bytes seeded on disk, the way the watcher leaves them, and the serving rule's
//! helpers: each case is a real request and its status and headers.

use crate::files::serving_rule::{
    get, header_value, served_as, test_app, test_app_with_permits, SeededFile, OWNER,
};
use anyhow::Result;
use axum::http::{header, StatusCode};
use nexus_common::media::{file_folder, FileVariant};
use nexus_common::models::{file::FileDetails, traits::Collection};
use pubky_social_specs::traits::HashId;
use pubky_social_specs::PubkySocialFile;
use std::path::Path;
use tempfile::TempDir;
use tokio::fs::{create_dir_all, read, try_exists, write};

/// A user of the mock graph (`docker/test-graph`) other than [`OWNER`], with a v0 file there.
const OTHER_OWNER: &str = "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy";
const OTHER_FILE_ID: &str = "2ZK1VCJN4YE00";

/// A real image, so ImageMagick can derive its variants.
const PNG_BLOB: &str = "tests/files/blobs/SynonymLogo.png";

/// A small valid SVG, enough for ImageMagick to rasterise.
const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="#00f"/></svg>"##;

/// A canonical hash id no bytes here hash to: blake3 over `[1, 2]`, first 16 bytes, Crockford.
const ABSENT_HASH: &str = "PZBQ010FF079VVZPQG1RNFN6DR";

/// A v0 id this suite owns, for the one `File` node it writes; no other suite uses the prefix.
const LEGACY_HTML_ID: &str = "2ZV1RTE000010";

/// The `{hash}.{ext}` of `bytes`, as the watcher names the file it writes.
fn v1_filename(bytes: &[u8], ext: &str) -> String {
    format!("{}.{ext}", PubkySocialFile(bytes.to_vec()).create_id())
}

/// Seeds `bytes` as the `main` of a v1 file of [`OWNER`] in the watcher's layout, and returns
/// the file's `{hash}.{ext}`.
async fn seed_v1(files_dir: &Path, ext: &str, bytes: &[u8]) -> Result<String> {
    let filename = v1_filename(bytes, ext);
    let folder = files_dir.join(file_folder(OWNER, &filename));
    create_dir_all(&folder).await?;
    write(folder.join(FileVariant::Main.to_string()), bytes).await?;
    Ok(filename)
}

fn v1_route(owner: &str, file_id: &str, variant: &str) -> String {
    format!("/v1/static/files/{owner}/{file_id}/{variant}")
}

// Both id forms are served from one path: a v1 file from the owner's v1 folder under the type
// its extension names, and a v0 file of another user from its node and legacy folder.
#[tokio_shared_rt::test(shared)]
async fn test_v1_route_serves_both_id_forms() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;

    let png = read(PNG_BLOB).await?;
    let filename = seed_v1(files_dir.path(), "png", &png).await?;

    let response = get(&app, &v1_route(OWNER, &filename, "main"), None).await?;
    assert_eq!(served_as(&response, "v1 png"), ("image/png", None));
    assert_eq!(
        header_value(&response, header::CONTENT_LENGTH),
        Some(png.len().to_string().as_str())
    );

    let legacy = FileDetails::get_by_ids(&[&[OTHER_OWNER, OTHER_FILE_ID]])
        .await?
        .pop()
        .flatten()
        .expect("fixture file in the graph");
    let legacy_folder = files_dir
        .path()
        .join(file_folder(OTHER_OWNER, OTHER_FILE_ID));
    create_dir_all(&legacy_folder).await?;
    write(legacy_folder.join("main"), b"legacy bytes").await?;

    let response = get(&app, &v1_route(OTHER_OWNER, OTHER_FILE_ID, "main"), None).await?;
    assert!(response.status().is_success(), "{}", response.status());
    assert_eq!(
        header_value(&response, header::CONTENT_TYPE),
        Some(legacy.content_type.as_str()),
        "a {{tsid}} is served under its node's type"
    );
    assert_eq!(
        header_value(&response, header::CONTENT_LENGTH),
        Some("12"),
        "and from its legacy folder"
    );
    Ok(())
}

// Every variant of a v1 image is served: `main` as uploaded, the derived ones as the WebP the
// processor writes, into the file's own v1 folder.
#[tokio_shared_rt::test(shared)]
async fn test_v1_route_serves_each_variant_of_a_v1_image() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;
    let filename = seed_v1(files_dir.path(), "png", &read(PNG_BLOB).await?).await?;

    for (variant, content_type) in [
        ("main", "image/png"),
        ("large", "image/webp"),
        ("feed", "image/webp"),
        ("small", "image/webp"),
    ] {
        let response = get(&app, &v1_route(OWNER, &filename, variant), None).await?;
        assert_eq!(served_as(&response, variant), (content_type, None));
        assert!(
            try_exists(
                files_dir
                    .path()
                    .join(file_folder(OWNER, &filename))
                    .join(variant)
            )
            .await?,
            "{variant} is written next to main"
        );
    }
    Ok(())
}

// An id of neither form is a 400: what the spec crate's reader does not accept as a v1 file
// name, and a v0 id of the wrong length. So is a variant the file's type lacks, with the same
// message as on `/static/files`.
#[tokio_shared_rt::test(shared)]
async fn test_v1_route_rejects_ids_of_neither_form_and_missing_variants() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;

    for file_id in [
        format!("{ABSENT_HASH}.JPG"),
        format!("{ABSENT_HASH}.tar.gz"),
        ABSENT_HASH.to_string(),
        "2ZK2H8P2T5NG".to_string(),
    ] {
        let response = get(&app, &v1_route(OWNER, &file_id, "main"), None).await?;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "{file_id}: an id of neither form"
        );
    }

    let video = seed_v1(
        files_dir.path(),
        "mp4",
        b"not inspected: the type is the extension",
    )
    .await?;
    let response = get(&app, &v1_route(OWNER, &video, "small"), None).await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), 1024).await?;
    assert!(
        String::from_utf8_lossy(&body).contains("not valid for content type video/mp4"),
        "{body:?}"
    );
    Ok(())
}

// A v1 file not on disk is a 404, not a request for a variant: with no media permit at all, a
// derived variant would shed with 503 if the request reached the gate, so the 404 shows it
// never did, and nothing was written for the file.
#[tokio_shared_rt::test(shared)]
async fn test_v1_route_missing_file_is_404_and_starts_no_conversion() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app_with_permits(files_dir.path(), 0).await;
    let filename = format!("{ABSENT_HASH}.png");

    for variant in ["main", "small"] {
        let response = get(&app, &v1_route(OWNER, &filename, variant), None).await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{variant}");
    }
    assert!(
        !try_exists(files_dir.path().join(file_folder(OWNER, &filename))).await?,
        "no folder is made for a file that is not there"
    );
    Ok(())
}

// The serving rule holds on the v1 form, where the type is the extension's: every inline type
// displays, with no `Content-Disposition`, and anything that can carry code or is not on the
// list is a bare download. `xml` is `application/xml` and `bin` is octet-stream, which downloads.
#[tokio_shared_rt::test(shared)]
async fn test_v1_route_serves_each_extension_under_the_serving_rule() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;

    // (extension, the content type it names, the Content-Disposition expected)
    let rows: &[(&str, &str, Option<&str>)] = &[
        ("jpg", "image/jpeg", None),
        ("png", "image/png", None),
        ("gif", "image/gif", None),
        ("webp", "image/webp", None),
        ("mp4", "video/mp4", None),
        ("mpeg", "video/mpeg", None),
        ("mp3", "audio/mpeg", None),
        ("wav", "audio/wav", None),
        ("pdf", "application/pdf", None),
        ("txt", "text/plain", None),
        ("html", "text/html", Some("attachment")),
        ("svg", "image/svg+xml", Some("attachment")),
        ("js", "application/javascript", Some("attachment")),
        ("css", "text/css", Some("attachment")),
        ("json", "application/json", Some("attachment")),
        ("xml", "application/xml", Some("attachment")),
        ("zip", "application/zip", Some("attachment")),
        ("csv", "text/csv", Some("attachment")),
        ("bin", "application/octet-stream", Some("attachment")),
    ];

    for (ext, content_type, disposition) in rows {
        // Distinct bytes per row, so each file has a hash of its own.
        let filename = seed_v1(files_dir.path(), ext, format!("{ext} bytes").as_bytes()).await?;
        let response = get(&app, &v1_route(OWNER, &filename, "main"), None).await?;
        assert_eq!(
            served_as(&response, ext),
            (*content_type, *disposition),
            "{ext}"
        );
    }
    Ok(())
}

// The rule reads the type of what is served: an SVG's `main` downloads, its `small` is the WebP
// the processor wrote, and displays.
#[tokio_shared_rt::test(shared)]
async fn test_v1_route_svg_main_downloads_and_its_derived_variant_displays() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;
    let filename = seed_v1(files_dir.path(), "svg", SVG.as_bytes()).await?;

    let main = get(&app, &v1_route(OWNER, &filename, "main"), None).await?;
    assert_eq!(
        served_as(&main, "svg main"),
        ("image/svg+xml", Some("attachment"))
    );

    let small = get(&app, &v1_route(OWNER, &filename, "small"), None).await?;
    assert_eq!(served_as(&small, "svg small"), ("image/webp", None));
    Ok(())
}

// The `{tsid}` form keeps the node's declared type and the rule on it: a parameterised,
// mixed-case HTML is HTML and downloads, through this route as through `/static/files`.
#[tokio_shared_rt::test(shared)]
async fn test_v1_route_legacy_mixed_case_html_downloads() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;
    let file = SeededFile::create(
        files_dir.path(),
        LEGACY_HTML_ID,
        "page.html",
        "Text/HTML; charset=x",
        b"<script>alert(1)</script>",
    )
    .await?;

    let outcome = async {
        let response = get(&app, &v1_route(OWNER, &file.details.id, "main"), None).await?;
        anyhow::ensure!(
            served_as(&response, "legacy html") == ("Text/HTML; charset=x", Some("attachment")),
            "a bare attachment under the declared type"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    file.remove().await?;
    outcome
}

// `?dl=` downloads a v1 file under its `{hash}.{ext}`, the only name it has, for a type that
// would display and for one that would not.
#[tokio_shared_rt::test(shared)]
async fn test_v1_route_dl_names_the_file_after_its_address() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;

    for (ext, bytes) in [("txt", &b"hi"[..]), ("html", &b"<p>hi</p>"[..])] {
        let filename = seed_v1(files_dir.path(), ext, bytes).await?;
        let response = get(
            &app,
            &format!("{}?dl", v1_route(OWNER, &filename, "main")),
            None,
        )
        .await?;
        let (_, disposition) = served_as(&response, ext);
        assert_eq!(
            disposition,
            Some(format!("attachment; filename=\"{filename}\"").as_str()),
            "{ext}"
        );
    }
    Ok(())
}

// A v1 video keeps streaming: a range request is a 206 with the requested bytes, inline, with
// `nosniff` like every other response.
#[tokio_shared_rt::test(shared)]
async fn test_v1_route_video_range_request_returns_206() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;
    let filename = seed_v1(files_dir.path(), "mp4", b"0123456789abcdef").await?;

    let response = get(&app, &v1_route(OWNER, &filename, "main"), Some("bytes=0-3")).await?;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(served_as(&response, "video range"), ("video/mp4", None));
    assert_eq!(
        header_value(&response, header::CONTENT_RANGE),
        Some("bytes 0-3/16")
    );
    Ok(())
}
