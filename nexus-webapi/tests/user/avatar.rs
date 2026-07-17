use std::{io::ErrorKind, path::PathBuf, sync::Arc, time::Duration};

use crate::utils::host_url;
use crate::utils::server::TestServiceServer;
use anyhow::Result;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use nexus_common::media::{MediaGate, VariantController};
use nexus_common::models::{traits::Collection, user::UserDetails};
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_common::RateLimitConfig;
use nexus_webapi::routes::{app_routes, build_app, AppState};
use tempfile::TempDir;
use tokio::fs::{create_dir_all, read, remove_dir_all, write};
use tokio::sync::watch;
use tower::ServiceExt;

const AVATAR_BLOB_NAME: &str = "avatar.png";
const BLOB_PATH: &str = "tests/user/blobs";

// User "Aldert" in docker/test-graph/skunk.cypher has u.image set to
// pubky://<USER_PUBKY>/pub/pubky.app/files/<FILE_ID>, and FILE_ID is
// defined in docker/test-graph/mocks/files.cypher.
//
// USER_PUBKY/FILE_ID is intentionally shared across the avatar tests: the
// handler resolves the file via FileDetails::get_by_ids, so it needs a real
// FileDetails that exists in the graph rather than a synthetic on-disk-only file.
const USER_PUBKY: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const FILE_ID: &str = "003286NSMY490";

// User "Intruder" in docker/test-graph/skunk.cypher has no profile image.
//
// Used as the profile fixture for the cross-owner avatar case. The test mutates
// only Intruder's `image` field and restores it, keeping the fixture footprint
// smaller than creating a synthetic user visible to aggregate tests.
const INTRUDER_PUBKY: &str = "4p1qa1ko7wuta4f1qm8io495cqsmefbgfp85wtnm9bj55gqbhjpo";

async fn seed_avatar_main_variant(owner_id: &str, file_id: &str) -> Result<usize> {
    let test_image_dir_path = TestServiceServer::get_test_server()
        .await
        .temp_dir
        .path()
        .join(owner_id)
        .join(file_id);
    let full_image_path = test_image_dir_path.join("main");

    create_dir_all(&test_image_dir_path).await?;
    let source_bytes = read(PathBuf::from(BLOB_PATH).join(AVATAR_BLOB_NAME)).await?;
    let source_size = source_bytes.len();
    write(&full_image_path, &source_bytes).await?;

    Ok(source_size)
}

async fn remove_avatar_variants(owner_id: &str, file_id: &str) -> Result<()> {
    let test_image_dir_path = TestServiceServer::get_test_server()
        .await
        .temp_dir
        .path()
        .join(owner_id)
        .join(file_id);

    match remove_dir_all(test_image_dir_path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

async fn set_user_image(user_id: &str, image: Option<String>) -> Result<()> {
    let mut user = UserDetails::get_by_id(user_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("user {user_id} not found in test graph"))?;
    user.image = image;
    user.put_to_graph().await?;
    UserDetails::put_to_index(&[user_id], vec![Some(user)]).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_avatar_endpoint() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let source_size = seed_avatar_main_variant(USER_PUBKY, FILE_ID).await?;

    let res = client
        .do_get(format!("/static/avatar/{USER_PUBKY}").as_str())
        .await?;

    assert_eq!(
        res.status(),
        200,
        "Should receive 200 OK serving the avatar"
    );

    assert_eq!(
        res.header("content-type")
            .unwrap()
            .parse::<String>()
            .unwrap(),
        "image/webp",
        "Avatar should be served as image/webp (small variant)"
    );

    assert_eq!(
        res.header("cache-control")
            .unwrap()
            .parse::<String>()
            .unwrap(),
        "public, max-age=3600",
        "Avatar response should set Cache-Control: public, max-age=3600"
    );

    let content_length: usize = res.header("content-length").unwrap().parse().unwrap();
    assert_ne!(
        content_length, source_size,
        "Served size should differ from source — proves small webp variant was generated, not the original served back"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_avatar_serves_file_owned_by_another_user() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let original_intruder = UserDetails::get_by_id(INTRUDER_PUBKY)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Intruder user not found in test graph"))?;
    let original_image = original_intruder.image.clone();

    let cross_owner_image = format!("pubky://{USER_PUBKY}/pub/pubky.app/files/{FILE_ID}");
    set_user_image(INTRUDER_PUBKY, Some(cross_owner_image)).await?;

    let test_result = async {
        // A regression to using the profile user's path would look here and fail.
        remove_avatar_variants(INTRUDER_PUBKY, FILE_ID).await?;

        // The blob exists only under the file owner's path.
        let source_size = seed_avatar_main_variant(USER_PUBKY, FILE_ID).await?;

        let res = client
            .do_get(format!("/static/avatar/{INTRUDER_PUBKY}").as_str())
            .await?;

        anyhow::ensure!(
            res.status() == 200,
            "Avatar should resolve the file owner's on-disk path, not the profile user's"
        );

        let content_type = res
            .header("content-type")
            .ok_or_else(|| anyhow::anyhow!("missing content-type header"))?
            .parse::<String>()?;
        anyhow::ensure!(
            content_type == "image/webp",
            "Avatar should be served as image/webp, got {content_type}"
        );

        let content_length = res
            .header("content-length")
            .ok_or_else(|| anyhow::anyhow!("missing content-length header"))?
            .parse::<usize>()?;
        anyhow::ensure!(
            content_length != source_size,
            "Served size should differ from source — proves the owned file was found and processed"
        );

        Ok::<(), anyhow::Error>(())
    }
    .await;

    let restore_result = set_user_image(INTRUDER_PUBKY, original_image).await;
    test_result?;
    restore_result?;

    Ok(())
}

// When the media concurrency gate is at capacity, the avatar handler must shed
// load by serving the untouched `main` image instead of failing with a 500.
#[tokio_shared_rt::test(shared)]
async fn test_user_avatar_degrades_to_main_when_at_capacity() -> Result<()> {
    // Ensure the shared stack (DB connectors) and test-graph data are initialized.
    // We drive a standalone router below so we don't disturb the shared server's gate.
    let _ = TestServiceServer::get_test_server().await;

    // Own static-files dir with only the `main` image present.
    let files_dir = TempDir::new()?;
    let image_dir = files_dir.path().join(USER_PUBKY).join(FILE_ID);
    create_dir_all(&image_dir).await?;
    let source_bytes = read(PathBuf::from(BLOB_PATH).join(AVATAR_BLOB_NAME)).await?;
    write(image_dir.join("main"), &source_bytes).await?;

    // A gate with zero permits and a short timeout: every variant generation
    // sheds with `AtCapacity` almost immediately.
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
                .uri(format!("/static/avatar/{USER_PUBKY}"))
                .body(Body::empty())?,
        )
        .await?;

    assert_eq!(
        res.status(),
        200,
        "At capacity should still serve 200 by falling back to main"
    );

    // FILE_ID's content type in the mock graph is image/jpeg (the main/original),
    // not image/webp (the small variant that would have been generated).
    let content_type = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        content_type, "image/jpeg",
        "Fallback should serve the original main image, not the small webp variant"
    );

    let body = to_bytes(res.into_body(), usize::MAX).await?;
    assert_eq!(
        body.len(),
        source_bytes.len(),
        "Fallback body should be the untouched main image bytes"
    );

    Ok(())
}

// With the gate below capacity, the same standalone router generates and serves
// the small webp variant — the counterpart to the at-capacity degradation above.
#[tokio_shared_rt::test(shared)]
async fn test_user_avatar_serves_small_variant_when_gate_available() -> Result<()> {
    let _ = TestServiceServer::get_test_server().await;

    let files_dir = TempDir::new()?;
    let image_dir = files_dir.path().join(USER_PUBKY).join(FILE_ID);
    create_dir_all(&image_dir).await?;
    let source_bytes = read(PathBuf::from(BLOB_PATH).join(AVATAR_BLOB_NAME)).await?;
    write(image_dir.join("main"), &source_bytes).await?;

    // A gate with an available permit lets variant generation proceed.
    let gate = MediaGate::new(1);
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
                .uri(format!("/static/avatar/{USER_PUBKY}"))
                .body(Body::empty())?,
        )
        .await?;

    assert_eq!(res.status(), 200, "Should serve 200 with the small variant");

    let content_type = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        content_type, "image/webp",
        "With capacity the generated small variant is served as image/webp"
    );

    let body = to_bytes(res.into_body(), usize::MAX).await?;
    assert_ne!(
        body.len(),
        source_bytes.len(),
        "Small variant bytes should differ from the original main image"
    );

    Ok(())
}
