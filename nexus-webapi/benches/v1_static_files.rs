//! `GET /v1/static/files/{owner_id}/{file_id}/{variant}` through the router, for both id forms:
//! a `{tsid}` read from its `File` node and legacy folder, and a `{hash}.{ext}` read from the
//! owner's v1 folder with no node, which adds a whole-address parse and a check that `main` is
//! on disk per request. Warm serves the `small` variant already on disk; cold removes it first,
//! so every request derives it again.

use std::{path::PathBuf, time::Duration};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use criterion::{criterion_group, criterion_main, Criterion};
use http_body_util::BodyExt;
use nexus_common::media::file_folder;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_common::RateLimitConfig;
use nexus_webapi::media::test_utils::default_subprocess_tests;
use nexus_webapi::media::MediaPermits;
use nexus_webapi::routes::{app_routes, build_app, AppState};
use pubky_social_specs::traits::HashId;
use pubky_social_specs::PubkySocialFile;
use tempfile::TempDir;
use tokio::{fs, runtime::Runtime, sync::watch};
use tower::ServiceExt;

mod setup;

use setup::run_setup;

/// A real image, so ImageMagick can derive its `small` variant.
const BLOB_PATH: &str = "tests/user/blobs/avatar.png";
/// A user of the mock graph with a `File` node (`docker/test-graph/mocks/files.cypher`): the
/// `{tsid}` form reads the node, so it must exist in the graph.
const OWNER: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const LEGACY_FILE_ID: &str = "003286NSMY490";

struct FilesBenchSetup {
    _temp_dir: TempDir,
    _shutdown_tx: watch::Sender<bool>,
    files_path: PathBuf,
    app: Router,
    v1_filename: String,
}

impl FilesBenchSetup {
    async fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let files_path = temp_dir.path().to_path_buf();
        let bytes = fs::read(BLOB_PATH).await.unwrap();

        let legacy_dir = files_path.join(file_folder(OWNER, LEGACY_FILE_ID));
        fs::create_dir_all(&legacy_dir).await.unwrap();
        fs::write(legacy_dir.join("main"), &bytes).await.unwrap();

        // Seeded as the watcher leaves a v1 file: `main` under `v1/<owner>/{hash}.{ext}`.
        let v1_filename = format!("{}.png", PubkySocialFile(bytes.clone()).create_id());
        let v1_dir = files_path.join(file_folder(OWNER, &v1_filename));
        fs::create_dir_all(&v1_dir).await.unwrap();
        fs::write(v1_dir.join("main"), &bytes).await.unwrap();

        let state = AppState::new(
            files_path.clone(),
            default_ingestor_tests(),
            MediaPermits::new(1),
            default_subprocess_tests(),
        );
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let routes = app_routes(state.clone(), &RateLimitConfig::default(), shutdown_rx);
        let app = build_app(routes, state, 30, 10 * 1024 * 1024);

        let setup = Self {
            _temp_dir: temp_dir,
            _shutdown_tx: shutdown_tx,
            files_path,
            app,
            v1_filename,
        };
        // One request per form first, so the first derivation is not in the warm numbers.
        setup.request(LEGACY_FILE_ID).await;
        setup.request(&setup.v1_filename).await;
        setup
    }

    async fn clear_small_variant(&self, file_id: &str) {
        let small = self
            .files_path
            .join(file_folder(OWNER, file_id))
            .join("small");
        if let Err(err) = fs::remove_file(small).await {
            if err.kind() != std::io::ErrorKind::NotFound {
                panic!("failed to remove small variant of {file_id}: {err}");
            }
        }
    }

    async fn request(&self, file_id: &str) {
        let uri = format!("/v1/static/files/{OWNER}/{file_id}/small");
        let response = self
            .app
            .clone()
            .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(!bytes.is_empty(), "{uri}: the body should not be empty");
        std::hint::black_box(bytes);
    }
}

fn bench_v1_static_files(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the /v1/static/files route through the router, without an HTTP server.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();
    // One TempDir for every case: PubkyServeDir keeps the first files_path in a process-global
    // OnceLock.
    let setup = rt.block_on(FilesBenchSetup::new());

    c.bench_function("v1_static_files_legacy_warm", |b| {
        b.to_async(&rt).iter(|| async {
            setup.request(LEGACY_FILE_ID).await;
        });
    });

    c.bench_function("v1_static_files_legacy_cold", |b| {
        b.to_async(&rt).iter(|| async {
            setup.clear_small_variant(LEGACY_FILE_ID).await;
            setup.request(LEGACY_FILE_ID).await;
        });
    });

    c.bench_function("v1_static_files_v1_warm", |b| {
        b.to_async(&rt).iter(|| async {
            setup.request(&setup.v1_filename).await;
        });
    });

    c.bench_function("v1_static_files_v1_cold", |b| {
        b.to_async(&rt).iter(|| async {
            setup.clear_small_variant(&setup.v1_filename).await;
            setup.request(&setup.v1_filename).await;
        });
    });
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(5, 0))
        .sample_size(100)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = v1_static_files;
    config = configure_criterion();
    targets = bench_v1_static_files
}

criterion_main!(v1_static_files);
