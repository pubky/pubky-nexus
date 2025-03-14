use std::{
    fs::{self},
    path::PathBuf,
};

use crate::utils::host_url;
use anyhow::Result;
use nexus_common::media::FileVariant;
use nexus_common::models::{file::FileDetails, traits::Collection};
use tokio::fs::create_dir_all;

const IMAGE_BLOB_NAME: &str = "SynonymLogo.png";
const BLOB_PATH: &str = "tests/files/blobs";

const FILE_ID: &str = "2ZKH7K7M9G3G0";
const USER_PUBKY: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

#[tokio_shared_rt::test(shared)]
async fn test_static_image_serving_main() -> Result<()> {
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
    // copy the image from mocks folder to static folder
    fs::copy(
        PathBuf::from(BLOB_PATH).join(IMAGE_BLOB_NAME),
        &full_image_path,
    )?;

    let files = FileDetails::get_by_ids(vec![vec![USER_PUBKY, FILE_ID].as_slice()].as_slice())
        .await
        .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    let test_file_version_name = FileVariant::Main.to_string();
    let test_file_path = format!("static/files/{USER_PUBKY}/{FILE_ID}/{test_file_version_name}");

    let res = client
        .do_get(format!("/{}", test_file_path.as_str()).as_str())
        .await?;

    assert_eq!(res.status(), 200);
    assert_eq!(
        res.header("content-type")
            .unwrap()
            .parse::<String>()
            .unwrap(),
        result_file.content_type
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_static_image_serving_feed() -> Result<()> {
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
    // copy the image from mocks folder to static folder
    fs::copy(
        PathBuf::from(BLOB_PATH).join(IMAGE_BLOB_NAME),
        &full_image_path,
    )?;

    let files = FileDetails::get_by_ids(vec![vec![USER_PUBKY, FILE_ID].as_slice()].as_slice())
        .await
        .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    let test_file_version_name = FileVariant::Feed.to_string();
    let test_file_path = format!("static/files/{USER_PUBKY}/{FILE_ID}/{test_file_version_name}");

    let res = client
        .do_get(format!("/{}", test_file_path.as_str()).as_str())
        .await?;

    assert_eq!(res.status(), 200);
    assert_ne!(
        res.header("content-type")
            .unwrap()
            .parse::<String>()
            .unwrap(),
        result_file.content_type
    );
    assert_eq!(
        res.header("content-type")
            .unwrap()
            .parse::<String>()
            .unwrap(),
        "image/webp"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_static_image_serving_small() -> Result<()> {
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
    // copy the image from mocks folder to static folder
    fs::copy(
        PathBuf::from(BLOB_PATH).join(IMAGE_BLOB_NAME),
        &full_image_path,
    )?;

    let files = FileDetails::get_by_ids(vec![vec![USER_PUBKY, FILE_ID].as_slice()].as_slice())
        .await
        .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    let test_file_version_name = FileVariant::Small.to_string();
    let test_file_path = format!("static/files/{USER_PUBKY}/{FILE_ID}/{test_file_version_name}");

    let res = client
        .do_get(format!("/{}", test_file_path.as_str()).as_str())
        .await?;

    assert_eq!(res.status(), 200);
    assert_ne!(
        res.header("content-type")
            .unwrap()
            .parse::<String>()
            .unwrap(),
        result_file.content_type
    );
    assert_eq!(
        res.header("content-type")
            .unwrap()
            .parse::<String>()
            .unwrap(),
        "image/webp"
    );

    Ok(())
}
