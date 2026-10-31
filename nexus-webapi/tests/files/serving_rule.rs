//! The serving rule on the file route: a file on the Nexus host never runs there. Every response
//! carries `nosniff`, a type a browser may render displays, and anything else is offered as a bare
//! download. Driven through the router with `File` nodes written to the graph and bytes seeded on
//! disk, so each row is a real request and its headers.

use crate::utils::server::TestServiceServer;
use anyhow::Result;
use axum::body::Body;
use axum::http::{header, Request, Response, StatusCode};
use axum::Router;
use nexus_common::models::{file::FileDetails, traits::Collection};
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_common::RateLimitConfig;
use nexus_webapi::media::test_utils::default_subprocess_tests;
use nexus_webapi::media::MediaPermits;
use nexus_webapi::routes::{app_routes, build_app, AppState};
use std::path::Path;
use tempfile::TempDir;
use tokio::fs::{create_dir_all, write};
use tokio::sync::watch;
use tower::ServiceExt;

/// An owner the mock graph knows (`docker/test-graph`). The files below are this suite's own,
/// written to the graph for one test and deleted after it.
const OWNER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

/// A small valid SVG, enough for ImageMagick to rasterise.
const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="#f00"/></svg>"##;

/// The ids this suite owns, one per `n`, so tests that run in parallel never share a node. A
/// 13-char Crockford id whose last char is `0`, as the mock ids are.
fn file_id(n: u32) -> String {
    format!("2ZSRVRLE{n:04}0")
}

/// A `File` node in the graph plus its `main` bytes under the router's own files folder. Dropped
/// from the graph by `remove`; the bytes go with the temp dir.
struct SeededFile {
    details: FileDetails,
}

impl SeededFile {
    async fn create(
        files_dir: &Path,
        id: &str,
        name: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        let details = FileDetails {
            id: id.to_string(),
            uri: format!("pubky://{OWNER}/pub/pubky.app/files/{id}"),
            owner_id: OWNER.to_string(),
            indexed_at: 0,
            created_at: 0,
            src: format!("pubky://{OWNER}/pub/pubky.app/blobs/{id}"),
            name: name.to_string(),
            size: bytes.len() as i64,
            content_type: content_type.to_string(),
            ..Default::default()
        };
        details.put_to_graph().await?;

        let folder = files_dir.join(OWNER).join(id);
        create_dir_all(&folder).await?;
        write(folder.join("main"), bytes).await?;

        Ok(Self { details })
    }

    fn path(&self, variant: &str) -> String {
        format!("/static/files/{OWNER}/{}/{variant}", self.details.id)
    }

    async fn remove(self) -> Result<()> {
        self.details.delete().await?;
        Ok(())
    }
}

/// A router over its own files folder, with one media permit so a derived variant can be made.
async fn test_app(files_dir: &Path) -> Router {
    // Initialises the shared DB connectors; the router below never touches the shared server.
    let _ = TestServiceServer::get_test_server().await;

    let state = AppState::new(
        files_dir.to_path_buf(),
        default_ingestor_tests(),
        MediaPermits::new(1),
        default_subprocess_tests(),
    );
    let (_tx, rx) = watch::channel(false);
    let routes = app_routes(state.clone(), &RateLimitConfig::default(), rx);
    build_app(routes, state, 30, 10 * 1024 * 1024)
}

async fn get(app: &Router, uri: &str, range: Option<&str>) -> Result<Response<Body>> {
    let mut request = Request::builder().uri(uri);
    if let Some(range) = range {
        request = request.header(header::RANGE, range);
    }
    Ok(app.clone().oneshot(request.body(Body::empty())?).await?)
}

fn header_value(response: &Response<Body>, name: header::HeaderName) -> Option<&str> {
    response.headers().get(name).and_then(|v| v.to_str().ok())
}

/// The headers the rule decides: `(content-type, content-disposition)`, after asserting the
/// response is a success that carries `nosniff`.
fn served_as<'a>(response: &'a Response<Body>, what: &str) -> (&'a str, Option<&'a str>) {
    assert!(
        response.status().is_success(),
        "{what}: expected a success, got {}",
        response.status()
    );
    assert_eq!(
        header_value(response, header::X_CONTENT_TYPE_OPTIONS),
        Some("nosniff"),
        "{what}: every file response carries nosniff"
    );
    (
        header_value(response, header::CONTENT_TYPE).unwrap_or_default(),
        header_value(response, header::CONTENT_DISPOSITION),
    )
}

// Each inline type displays, with no `Content-Disposition`; HTML, SVG, a script type, a malformed
// type and a parameterised, mixed-case HTML get a bare `attachment`. All through `main`, which is
// served under the declared type, so no conversion runs.
#[tokio_shared_rt::test(shared)]
async fn test_main_displays_inline_types_and_downloads_the_rest() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;

    // (content type as the node declares it, the Content-Disposition expected)
    let rows: &[(&str, Option<&str>)] = &[
        ("image/jpeg", None),
        ("image/png", None),
        ("image/gif", None),
        ("image/webp", None),
        ("video/mp4", None),
        ("video/mpeg", None),
        ("audio/mpeg", None),
        ("audio/wav", None),
        ("application/pdf", None),
        ("text/plain", None),
        ("text/html", Some("attachment")),
        ("image/svg+xml", Some("attachment")),
        ("application/javascript", Some("attachment")),
        ("garbage", Some("attachment")),
        ("Text/HTML; charset=x", Some("attachment")),
    ];

    let mut seeded = Vec::new();
    for (n, (content_type, _)) in rows.iter().enumerate() {
        seeded.push(
            SeededFile::create(
                files_dir.path(),
                &file_id(n as u32),
                "file",
                content_type,
                b"not inspected: the type comes from the node",
            )
            .await?,
        );
    }

    let outcome = async {
        for (file, (content_type, disposition)) in seeded.iter().zip(rows) {
            let response = get(&app, &file.path("main"), None).await?;
            let (served_type, served_disposition) = served_as(&response, content_type);
            anyhow::ensure!(
                served_type == *content_type,
                "{content_type}: main is served under the declared type, got {served_type}"
            );
            anyhow::ensure!(
                served_disposition == *disposition,
                "{content_type}: expected {disposition:?}, got {served_disposition:?}"
            );
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    for file in seeded {
        file.remove().await?;
    }
    outcome
}

// The rule reads the type of what is served, not what was uploaded: an SVG's `main` is SVG and
// downloads, while its `small` variant is the WebP the processor wrote, and displays.
#[tokio_shared_rt::test(shared)]
async fn test_svg_main_downloads_and_its_derived_variant_displays() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;
    let file = SeededFile::create(
        files_dir.path(),
        &file_id(20),
        "drawing.svg",
        "image/svg+xml",
        SVG.as_bytes(),
    )
    .await?;

    let outcome = async {
        let main = get(&app, &file.path("main"), None).await?;
        anyhow::ensure!(
            served_as(&main, "svg main") == ("image/svg+xml", Some("attachment")),
            "svg main downloads under its own type"
        );

        let small = get(&app, &file.path("small"), None).await?;
        anyhow::ensure!(
            served_as(&small, "svg small") == ("image/webp", None),
            "the derived variant is webp and displays"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    file.remove().await?;
    outcome
}

// A forced download names no file, so a name the header could not carry never fails the
// response; `?dl=` keeps naming the file as it does today.
#[tokio_shared_rt::test(shared)]
async fn test_non_ascii_name_does_not_fail_a_forced_download() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;
    let file = SeededFile::create(
        files_dir.path(),
        &file_id(21),
        "résumé.html",
        "text/html",
        b"<script>alert(1)</script>",
    )
    .await?;

    let outcome = async {
        let response = get(&app, &file.path("main"), None).await?;
        anyhow::ensure!(
            served_as(&response, "non-ascii name") == ("text/html", Some("attachment")),
            "a bare attachment, whatever the file is named"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    file.remove().await?;
    outcome
}

// `?dl=` is unchanged by the rule: it downloads under the file's name, for a type that would
// display and for one that would not.
#[tokio_shared_rt::test(shared)]
async fn test_dl_param_keeps_the_file_name() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;
    let plain = SeededFile::create(
        files_dir.path(),
        &file_id(22),
        "notes.txt",
        "text/plain",
        b"hi",
    )
    .await?;
    let page = SeededFile::create(
        files_dir.path(),
        &file_id(23),
        "page.html",
        "text/html",
        b"<p>hi</p>",
    )
    .await?;

    let outcome = async {
        for (file, name) in [(&plain, "notes.txt"), (&page, "page.html")] {
            let response = get(&app, &format!("{}?dl", file.path("main")), None).await?;
            let (_, disposition) = served_as(&response, name);
            anyhow::ensure!(
                disposition == Some(format!("attachment; filename=\"{name}\"").as_str()),
                "{name}: ?dl= names the file, got {disposition:?}"
            );
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    plain.remove().await?;
    page.remove().await?;
    outcome
}

// A video keeps streaming: a range request is a 206 with the requested bytes, inline, and
// carries `nosniff` like every other response.
#[tokio_shared_rt::test(shared)]
async fn test_video_range_request_returns_206() -> Result<()> {
    let files_dir = TempDir::new()?;
    let app = test_app(files_dir.path()).await;
    let video = SeededFile::create(
        files_dir.path(),
        &file_id(24),
        "clip.mp4",
        "video/mp4",
        b"0123456789abcdef",
    )
    .await?;

    let outcome = async {
        let response = get(&app, &video.path("main"), Some("bytes=0-3")).await?;
        anyhow::ensure!(
            response.status() == StatusCode::PARTIAL_CONTENT,
            "a range request is a 206, got {}",
            response.status()
        );
        anyhow::ensure!(
            served_as(&response, "video range") == ("video/mp4", None),
            "the partial video displays inline"
        );
        anyhow::ensure!(
            header_value(&response, header::CONTENT_RANGE) == Some("bytes 0-3/16"),
            "content-range names the served bytes, got {:?}",
            header_value(&response, header::CONTENT_RANGE)
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    video.remove().await?;
    outcome
}
