use std::path::PathBuf;

use crate::utils::host_url;
use crate::utils::server::TestServiceServer;
use anyhow::Result;
use tokio::fs::{create_dir_all, read, write};

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

    let test_image_dir_path = TestServiceServer::get_test_server()
        .await
        .temp_dir
        .path()
        .join(USER_PUBKY)
        .join(FILE_ID);
    let full_image_path = test_image_dir_path.join("main");

    create_dir_all(&test_image_dir_path).await?;
    // drop the source avatar into the static folder so the handler can
    // lazily generate the small webp variant from it
    let source_bytes = read(PathBuf::from(BLOB_PATH).join(AVATAR_BLOB_NAME)).await?;
    let source_size = source_bytes.len();
    write(&full_image_path, &source_bytes).await?;

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

    let content_length: usize = res
        .header("content-length")
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(
        content_length, source_size,
        "Served size should differ from source — proves small webp variant was generated, not the original served back"
    );

    Ok(())
}
