use std::sync::Arc;
use std::time::Duration;
use std::{fs::File, io::Write};

use crate::utils::host_url;
use crate::utils::server::TestServiceServer;
use anyhow::Result;
use axum::body::Body;
use axum::http::Request;
use nexus_common::media::{MediaGate, VariantController};
use nexus_common::models::{file::FileDetails, traits::Collection};
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_common::RateLimitConfig;
use nexus_webapi::routes::{app_routes, build_app, AppState};
use tempfile::TempDir;
use tokio::fs::{create_dir_all, write};
use tokio::sync::watch;
use tower::ServiceExt;

#[tokio_shared_rt::test(shared)]
async fn test_static_serving() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let test_file_id = "2ZK2H8P2T5NG0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

    let files =
        FileDetails::get_by_ids(vec![vec![test_file_user, test_file_id].as_slice()].as_slice())
            .await
            .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    let test_file_dir = TestServiceServer::get_test_server()
        .await
        .temp_dir
        .path()
        .join(test_file_user)
        .join(test_file_id);
    let test_file_name = "main";

    let full_file_path = test_file_dir.join(test_file_name);

    create_dir_all(&test_file_dir).await?;

    let mut file = File::create(&full_file_path)?;
    file.write_all(b"Hello, world!")?;

    let url_path = format!("static/files/{test_file_user}/{test_file_id}/{test_file_name}");
    let res = client.do_get(&format!("/{url_path}")).await?;

    assert_eq!(res.status(), 200);
    assert_eq!(
        res.header("content-length")
            .unwrap()
            .parse::<i64>()
            .unwrap(),
        result_file.size
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_static_serving_dl_param() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let test_file_id = "2ZKH7K7B2RY00";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

    let files =
        FileDetails::get_by_ids(vec![vec![test_file_user, test_file_id].as_slice()].as_slice())
            .await
            .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    let test_file_dir = TestServiceServer::get_test_server()
        .await
        .temp_dir
        .path()
        .join(test_file_user)
        .join(test_file_id);
    let test_file_name = "main";

    let full_file_path = test_file_dir.join(test_file_name);

    create_dir_all(&test_file_dir).await?;

    let mut file = File::create(&full_file_path)?;
    file.write_all(b"Hello, world!")?;

    let url_path = format!("static/files/{test_file_user}/{test_file_id}/{test_file_name}");
    let res = client.do_get(&format!("/{url_path}?dl")).await?;

    assert_eq!(res.status(), 200);
    assert_eq!(
        res.header("content-length")
            .unwrap()
            .parse::<i64>()
            .unwrap(),
        result_file.size
    );
    assert_eq!(
        res.header("content-disposition")
            .unwrap()
            .parse::<String>()
            .unwrap(),
        format!("attachment; filename=\"{}\"", result_file.name)
    );

    Ok(())
}

// Requesting a variant that must be generated while the media gate is at capacity
// must shed with 503 — the explicitly requested variant is not silently substituted.
#[tokio_shared_rt::test(shared)]
async fn test_static_serving_at_capacity_returns_503() -> Result<()> {
    // Ensure the shared stack (DB connectors) and test-graph data are initialized.
    // We drive a standalone router so we don't disturb the shared server's gate.
    let _ = TestServiceServer::get_test_server().await;

    // An image file from the mock graph (content_type image/png) that supports the feed variant.
    let test_file_id = "2ZKH7K7M9G3G0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

    let files_dir = TempDir::new()?;
    let image_dir = files_dir.path().join(test_file_user).join(test_file_id);
    create_dir_all(&image_dir).await?;
    write(
        image_dir.join("main"),
        b"not a real image, gate sheds first",
    )
    .await?;

    // Zero permits + short timeout: variant generation sheds with AtCapacity immediately.
    let gate = MediaGate::new(0).with_acquire_timeout(Duration::from_millis(50));
    let state = AppState {
        files_path: Arc::new(files_dir.path().to_path_buf()),
        ingestor: default_ingestor_tests(),
        variant_controller: VariantController::new(gate),
    };
    let (_tx, rx) = watch::channel(false);
    let routes = app_routes(state.clone(), &RateLimitConfig::default(), rx);
    let app = build_app(routes, state, 30, 10 * 1024 * 1024);

    let res = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/static/files/{test_file_user}/{test_file_id}/feed"
                ))
                .body(Body::empty())?,
        )
        .await?;

    assert_eq!(
        res.status(),
        503,
        "At capacity the requested variant must shed with 503, not fall back or 500"
    );

    Ok(())
}
