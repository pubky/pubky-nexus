use std::{
    fs::{self},
    path::PathBuf,
};

use crate::utils::host_url;
use anyhow::Result;
use tokio::fs::create_dir_all;

const AVATAR_BLOB_NAME: &str = "avatar.png";
const BLOB_PATH: &str = "tests/user/blobs";

// User "Aldert" in docker/test-graph/skunk.cypher has u.image set to
// pubky://<USER_PUBKY>/pub/pubky.app/files/<FILE_ID>, and FILE_ID is
// defined in docker/test-graph/mocks/files.cypher.
const USER_PUBKY: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const FILE_ID: &str = "003286NSMY490";

#[tokio_shared_rt::test(shared)]
async fn test_user_avatar_endpoint() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let test_image_dir_path = format!("static/files/{USER_PUBKY}/{FILE_ID}");
    let full_image_path = format!("{}/main", test_image_dir_path.clone());

    // make sure directory exists
    let exists = match fs::metadata(test_image_dir_path.clone()) {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };
    if !exists {
        create_dir_all(&test_image_dir_path).await?;
    }
    // copy the avatar image into the static folder so the avatar handler
    // can lazily generate the small webp variant from it
    fs::copy(
        PathBuf::from(BLOB_PATH).join(AVATAR_BLOB_NAME),
        &full_image_path,
    )?;

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

    Ok(())
}
