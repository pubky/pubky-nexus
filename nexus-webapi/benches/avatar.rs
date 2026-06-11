use std::{
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{Request, StatusCode},
};
use criterion::{criterion_group, criterion_main, Criterion};
use http_body_util::BodyExt;
use nexus_common::models::{
    file::{FileDetails, FileUrls},
    traits::Collection,
    user::UserDetails,
};
use nexus_webapi::{
    models::PubkyId,
    routes::{r#static::user_avatar_handler, AppState, Path},
};
use tempfile::TempDir;
use tokio::{fs, runtime::Runtime};

mod setup;

use setup::run_setup;

const AVATAR_BLOB_NAME: &str = "avatar.png";
const BLOB_PATH: &str = "tests/user/blobs";
const USER_PUBKY: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const FILE_ID: &str = "003286NSMY490";
const AVATAR_URI: &str =
    "pubky://4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro/pub/pubky.app/files/003286NSMY490";

struct AvatarBenchSetup {
    _temp_dir: TempDir,
    app_state: AppState,
    user_id: PubkyId,
}

impl AvatarBenchSetup {
    async fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let files_path = temp_dir.path().to_path_buf();
        let image_dir = files_path.join(USER_PUBKY).join(FILE_ID);
        fs::create_dir_all(&image_dir).await.unwrap();

        let source_path = PathBuf::from(BLOB_PATH).join(AVATAR_BLOB_NAME);
        let source_size = fs::copy(source_path, image_dir.join("main")).await.unwrap();

        Self::seed_avatar_records(source_size).await;

        let setup = Self {
            _temp_dir: temp_dir,
            app_state: AppState {
                files_path: Arc::new(files_path),
            },
            user_id: PubkyId::try_from(USER_PUBKY).unwrap(),
        };

        setup.warm_small_variant().await;
        setup
    }

    async fn seed_avatar_records(source_size: u64) {
        let user_id = PubkyId::try_from(USER_PUBKY).unwrap();
        let user = UserDetails {
            name: "Avatar Bench User".to_string(),
            bio: None,
            id: user_id,
            links: None,
            status: None,
            image: Some(AVATAR_URI.to_string()),
            indexed_at: 1_724_134_095_000,
        };

        UserDetails::put_to_index(&[USER_PUBKY], vec![Some(user)])
            .await
            .unwrap();

        let file = FileDetails {
            id: FILE_ID.to_string(),
            uri: AVATAR_URI.to_string(),
            owner_id: USER_PUBKY.to_string(),
            indexed_at: 1_724_134_095_000,
            created_at: 1_784_134_095_000,
            src: format!("pubky://{USER_PUBKY}/pub/pubky.app/blobs/{FILE_ID}"),
            name: AVATAR_BLOB_NAME.to_string(),
            size: source_size as i64,
            content_type: "image/png".to_string(),
            urls: FileUrls {
                main: format!("{USER_PUBKY}/{FILE_ID}"),
                feed: None,
                small: None,
            },
            metadata: None,
        };

        FileDetails::put_to_index(&[&[USER_PUBKY, FILE_ID]], vec![Some(file)])
            .await
            .unwrap();
    }

    async fn warm_small_variant(&self) {
        let response = self.call_avatar_handler().await.unwrap();
        std::hint::black_box(consume_avatar_response(response).await);
    }

    async fn clear_small_variant(&self) {
        let small_variant_path = self
            .app_state
            .files_path
            .join(USER_PUBKY)
            .join(FILE_ID)
            .join("small");

        if let Err(err) = fs::remove_file(small_variant_path).await {
            if err.kind() != std::io::ErrorKind::NotFound {
                panic!("failed to remove small avatar variant: {err}");
            }
        }
    }

    async fn call_avatar_handler(&self) -> nexus_webapi::Result<
        axum::response::Response<tower_http::services::fs::ServeFileSystemResponseBody>,
    > {
        user_avatar_handler(
            Path(self.user_id.clone()),
            State(self.app_state.clone()),
            Request::new(Body::empty()),
        )
        .await
    }
}

async fn consume_avatar_response(
    response: axum::response::Response<tower_http::services::fs::ServeFileSystemResponseBody>,
) -> (StatusCode, Bytes) {
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();

    assert_eq!(status, StatusCode::OK);
    assert!(!bytes.is_empty(), "avatar response body should not be empty");

    (status, bytes)
}

fn bench_avatar_handler(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking avatar handler chain without an HTTP server.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();
    // PubkyServeDir caches the first files_path in a process-wide OnceLock, so both
    // measurements must share one temp directory while this function is running.
    let setup = rt.block_on(AvatarBenchSetup::new());

    c.bench_function("avatar_handler_warm", |b| {
        b.to_async(&rt).iter(|| async {
            let response = setup.call_avatar_handler().await.unwrap();
            std::hint::black_box(consume_avatar_response(response).await);
        });
    });

    c.bench_function("avatar_handler_cold", |b| {
        b.to_async(&rt).iter(|| async {
            setup.clear_small_variant().await;

            let response = setup.call_avatar_handler().await.unwrap();
            std::hint::black_box(consume_avatar_response(response).await);
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
    name = avatar;
    config = configure_criterion();
    targets = bench_avatar_handler
}

criterion_main!(avatar);
