use std::{io::ErrorKind, path::PathBuf};

use crate::utils::host_url;
use crate::utils::server::TestServiceServer;
use anyhow::Result;
use nexus_common::models::{traits::Collection, user::UserDetails};
use tokio::fs::{create_dir_all, read, remove_dir_all, write};

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
