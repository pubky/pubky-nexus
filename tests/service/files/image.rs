use std::fs::{self};

use anyhow::Result;
use pubky_nexus::models::{
    file::{details::FileVariant, FileDetails},
    traits::Collection,
};
use tokio::fs::create_dir_all;

use crate::service::utils::host_url;

#[tokio_shared_rt::test(shared)]
async fn test_static_image_serving_main() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let test_file_id = "2ZKH7K7M9G3G0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let test_image_blob_name = "SynonymLogo.png";

    let test_image_dir_path = format!("static/files/{test_file_user}/{test_file_id}");
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
        format!("docker/db-graph/mocks/blobs/{test_image_blob_name}"),
        &full_image_path,
    )?;

    let files =
        FileDetails::get_by_ids(vec![vec![test_file_user, test_file_id].as_slice()].as_slice())
            .await
            .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    let test_file_version_name = FileVariant::Main.to_string();
    let test_file_path =
        format!("static/files/{test_file_user}/{test_file_id}/{test_file_version_name}");

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

    let test_file_id = "2ZKH7K7M9G3G0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let test_image_blob_name = "SynonymLogo.png";

    let test_image_dir_path = format!("static/files/{test_file_user}/{test_file_id}");
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
        format!("docker/db-graph/mocks/blobs/{test_image_blob_name}"),
        &full_image_path,
    )?;

    let files =
        FileDetails::get_by_ids(vec![vec![test_file_user, test_file_id].as_slice()].as_slice())
            .await
            .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    let test_file_version_name = FileVariant::Feed.to_string();
    let test_file_path =
        format!("static/files/{test_file_user}/{test_file_id}/{test_file_version_name}");

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

    let test_file_id = "2ZKH7K7M9G3G0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let test_image_blob_name = "SynonymLogo.png";

    let test_image_dir_path = format!("static/files/{test_file_user}/{test_file_id}");
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
        format!("docker/db-graph/mocks/blobs/{test_image_blob_name}"),
        &full_image_path,
    )?;

    let files =
        FileDetails::get_by_ids(vec![vec![test_file_user, test_file_id].as_slice()].as_slice())
            .await
            .expect("Failed to fetch files from Nexus");

    let result_file = files[0].as_ref().expect("Created file was not found.");

    let test_file_version_name = FileVariant::Small.to_string();
    let test_file_path =
        format!("static/files/{test_file_user}/{test_file_id}/{test_file_version_name}");

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
