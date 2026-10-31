use std::{io::ErrorKind, path::PathBuf, time::Duration, time::Instant};

use crate::utils::host_url;
use crate::utils::server::TestServiceServer;
use anyhow::{ensure, Result};
use axum::body::{to_bytes, Body};
use axum::http::Request;
use nexus_common::media::file_folder;
use nexus_common::models::{traits::Collection, user::UserDetails};
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_common::RateLimitConfig;
use nexus_webapi::media::test_utils::default_subprocess_tests;
use nexus_webapi::media::MediaPermits;
use nexus_webapi::routes::{app_routes, build_app, AppState};
use pubky_social_specs::traits::HashId;
use pubky_social_specs::{file_uri_builder, PubkySocialFile};
use tempfile::TempDir;
use tokio::fs::{create_dir_all, read, remove_dir_all, try_exists, write};
use tokio::sync::watch;
use tower::ServiceExt;

const AVATAR_BLOB_NAME: &str = "avatar.png";
const BLOB_PATH: &str = "tests/user/blobs";

/// A second real image, so a second v1 file has a hash of its own.
const OTHER_PNG_BLOB: &str = "tests/files/blobs/SynonymLogo.png";

/// The two mounts of the one avatar handler: the v0 path and the v1 base path.
const AVATAR_MOUNTS: [&str; 2] = ["/static/avatar", "/v1/static/avatar"];

/// A canonical hash id no bytes here hash to: blake3 over `[1, 2]`, first 16 bytes, Crockford.
const ABSENT_HASH: &str = "PZBQ010FF079VVZPQG1RNFN6DR";

/// A post of the mock graph, for an address that is not a file.
const POST_ID: &str = "0032SSN7Q4EVG";

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
// The one profile the avatar table is driven through: only its `image` field is set, per row,
// and restored at the end, keeping the fixture footprint smaller than creating a synthetic user
// visible to aggregate tests. One test mutates it, so no two rows can race.
const INTRUDER_PUBKY: &str = "4p1qa1ko7wuta4f1qm8io495cqsmefbgfp85wtnm9bj55gqbhjpo";

/// The shared server's files folder, the one its avatar handler reads.
async fn shared_files_dir() -> PathBuf {
    TestServiceServer::get_test_server()
        .await
        .temp_dir
        .path()
        .to_path_buf()
}

async fn seed_avatar_main_variant(owner_id: &str, file_id: &str) -> Result<usize> {
    let test_image_dir_path = shared_files_dir().await.join(owner_id).join(file_id);
    let full_image_path = test_image_dir_path.join("main");

    create_dir_all(&test_image_dir_path).await?;
    let source_bytes = read(PathBuf::from(BLOB_PATH).join(AVATAR_BLOB_NAME)).await?;
    let source_size = source_bytes.len();
    write(&full_image_path, &source_bytes).await?;

    Ok(source_size)
}

async fn remove_avatar_variants(owner_id: &str, file_id: &str) -> Result<()> {
    let test_image_dir_path = shared_files_dir().await.join(owner_id).join(file_id);

    match remove_dir_all(test_image_dir_path).await {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Seeds `bytes` as the `main` of a `social/v1` file of `owner_id` in the watcher's layout, under
/// the shared server's files folder, and returns the file's `{hash}.{ext}` and its address.
async fn seed_v1_file(owner_id: &str, ext: &str, bytes: &[u8]) -> Result<(String, String)> {
    let filename = format!("{}.{ext}", PubkySocialFile(bytes.to_vec()).create_id());
    let folder = shared_files_dir()
        .await
        .join(file_folder(owner_id, &filename));
    create_dir_all(&folder).await?;
    write(folder.join("main"), bytes).await?;
    let uri = file_uri_builder(owner_id.to_string(), filename.clone());
    Ok((filename, uri))
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

// A v0 file address is served as its small WebP variant, through both mounts, under the serving
// rule: inline, and nothing is sniffed.
#[tokio_shared_rt::test(shared)]
async fn test_user_avatar_endpoint() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let source_size = seed_avatar_main_variant(USER_PUBKY, FILE_ID).await?;

    for mount in AVATAR_MOUNTS {
        let res = client
            .do_get(format!("{mount}/{USER_PUBKY}").as_str())
            .await?;

        assert_eq!(
            res.status(),
            200,
            "{mount}: should receive 200 OK serving the avatar"
        );

        assert_eq!(
            res.header("content-type")
                .unwrap()
                .parse::<String>()
                .unwrap(),
            "image/webp",
            "{mount}: avatar should be served as image/webp (small variant)"
        );

        assert_eq!(
            res.header("cache-control")
                .unwrap()
                .parse::<String>()
                .unwrap(),
            "public, max-age=3600",
            "{mount}: avatar response should set Cache-Control: public, max-age=3600"
        );

        let content_length: usize = res.header("content-length").unwrap().parse().unwrap();
        assert_ne!(
            content_length, source_size,
            "{mount}: served size should differ from source — proves small webp variant was generated, not the original served back"
        );

        // The avatar goes through the same serve function as the file route, so it carries the
        // serving rule too: an image displays inline, and nothing is sniffed.
        assert_eq!(
            res.header("x-content-type-options").as_deref(),
            Some("nosniff"),
            "{mount}"
        );
        assert!(
            res.header("content-disposition").is_none(),
            "{mount}: an avatar image is not offered as a download"
        );
    }

    Ok(())
}

// Every row of the avatar table, through both mounts, on one profile whose `image` is set per
// row. An image file Nexus holds is served as its small variant from the file owner's folder,
// legacy or v1, the owner read off the address; anything else is a 404, never a redirect or a
// fetch. A v1 image not on disk is a 404 that starts no conversion.
#[tokio_shared_rt::test(shared)]
async fn test_user_avatar_serves_each_row_of_the_image_table() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;
    let files_dir = shared_files_dir().await;

    let original_image = UserDetails::get_by_id(INTRUDER_PUBKY)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Intruder user not found in test graph"))?
        .image;

    // Another user's v0 file: the blob exists only under the file owner's legacy folder, so a
    // regression to the profile user's path would look in an empty place and fail.
    remove_avatar_variants(INTRUDER_PUBKY, FILE_ID).await?;
    seed_avatar_main_variant(USER_PUBKY, FILE_ID).await?;
    let legacy_other = format!("pubky://{USER_PUBKY}/pub/pubky.app/files/{FILE_ID}");

    // The profile user's own v1 image, another user's v1 image (another real image, so another
    // hash, seeded only under that user), a v1 file that is no image, and a v1 image not on disk.
    let avatar_png = read(PathBuf::from(BLOB_PATH).join(AVATAR_BLOB_NAME)).await?;
    let (own_name, own_v1) = seed_v1_file(INTRUDER_PUBKY, "png", &avatar_png).await?;
    let (other_name, other_v1) =
        seed_v1_file(USER_PUBKY, "png", &read(OTHER_PNG_BLOB).await?).await?;
    let (_, not_an_image) =
        seed_v1_file(INTRUDER_PUBKY, "pdf", b"%PDF-1.4 bytes that are no image").await?;
    let missing_name = format!("{ABSENT_HASH}.png");
    let missing_v1 = file_uri_builder(INTRUDER_PUBKY.to_string(), missing_name.clone());

    // (the case, the profile's `image`, the status both mounts answer)
    let rows: Vec<(&str, Option<String>, u16)> = vec![
        ("another user's v0 file", Some(legacy_other), 200),
        ("own v1 image", Some(own_v1), 200),
        ("another user's v1 image", Some(other_v1), 200),
        (
            "a web URL",
            Some("https://example.com/avatar.png".to_string()),
            404,
        ),
        (
            "another app's object",
            Some(format!(
                "pubky://{INTRUDER_PUBKY}/pub/other.app/v1/files/{ABSENT_HASH}.png"
            )),
            404,
        ),
        (
            "a v1 address that is not a file",
            Some(format!(
                "pubky://{INTRUDER_PUBKY}/pub/social/v1/posts/{POST_ID}"
            )),
            404,
        ),
        (
            "a v0 address that is not a file",
            Some(format!(
                "pubky://{INTRUDER_PUBKY}/pub/pubky.app/posts/{POST_ID}"
            )),
            404,
        ),
        ("a v1 file that is not an image", Some(not_an_image), 404),
        ("a v1 image not on disk", Some(missing_v1), 404),
        ("unset", None, 404),
    ];

    let outcome = async {
        for (what, image, status) in rows {
            set_user_image(INTRUDER_PUBKY, image).await?;
            for mount in AVATAR_MOUNTS {
                let res = client
                    .do_get(format!("{mount}/{INTRUDER_PUBKY}").as_str())
                    .await?;
                ensure!(
                    res.status() == status,
                    "{what} via {mount}: expected {status}, got {}",
                    res.status()
                );
                if status == 200 {
                    ensure!(
                        res.header("content-type").as_deref() == Some("image/webp"),
                        "{what} via {mount}: served as the small WebP variant, got {:?}",
                        res.header("content-type")
                    );
                }
            }
        }

        // Served from the file owner's folder: the small variant each request derived sits next
        // to the owner's `main`, legacy or v1, and nothing was made under the profile user's.
        ensure!(
            try_exists(files_dir.join(USER_PUBKY).join(FILE_ID).join("small")).await?,
            "a v0 file's variant is derived in its owner's legacy folder"
        );
        ensure!(
            try_exists(
                files_dir
                    .join(file_folder(INTRUDER_PUBKY, &own_name))
                    .join("small")
            )
            .await?,
            "an own v1 image's variant is derived in the user's v1 folder"
        );
        ensure!(
            try_exists(
                files_dir
                    .join(file_folder(USER_PUBKY, &other_name))
                    .join("small")
            )
            .await?,
            "another user's v1 image is served from that user's v1 folder"
        );
        ensure!(
            !try_exists(files_dir.join(file_folder(INTRUDER_PUBKY, &other_name))).await?,
            "and nothing is made for it under the profile user"
        );
        ensure!(
            !try_exists(files_dir.join(file_folder(INTRUDER_PUBKY, &missing_name))).await?,
            "a v1 image not on disk starts no conversion: no folder is made for it"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    let restore_result = set_user_image(INTRUDER_PUBKY, original_image).await;
    outcome?;
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

    // Zero permits: every variant generation sheds with `AtCapacity`. The default
    // acquire timeout stays in place on purpose, so the assertion below catches a
    // fallback that queues for it instead of failing fast.
    let state = AppState::new(
        files_dir.path().to_path_buf(),
        default_ingestor_tests(),
        MediaPermits::new(0),
        default_subprocess_tests(),
    );
    let (_tx, rx) = watch::channel(false);
    let routes = app_routes(state.clone(), &RateLimitConfig::default(), rx);
    let app = build_app(routes, state, 30, 10 * 1024 * 1024);

    let start = Instant::now();
    let res = app
        .oneshot(
            Request::builder()
                .uri(format!("/static/avatar/{USER_PUBKY}"))
                .body(Body::empty())?,
        )
        .await?;
    let elapsed = start.elapsed();

    assert_eq!(
        res.status(),
        200,
        "At capacity should still serve 200 by falling back to main"
    );

    // The fallback must not wait out the gate's acquire timeout to reach the same answer.
    assert!(
        elapsed < Duration::from_secs(1),
        "At-capacity fallback must be immediate, took {elapsed:?}"
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

    // An available permit lets variant generation proceed.
    let state = AppState::new(
        files_dir.path().to_path_buf(),
        default_ingestor_tests(),
        MediaPermits::new(1),
        default_subprocess_tests(),
    );
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
