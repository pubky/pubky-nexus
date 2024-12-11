use std::fs::{self, create_dir_all, remove_dir_all};

use anyhow::Result;
use pubky_nexus::{
    models::{file::FileDetails, traits::Collection},
    setup, Config,
};

use crate::service::utils::HOST_URL;

#[tokio::test]
async fn test_static_image_serving_main() -> Result<()> {
    setup(&Config::from_env()).await;
    let test_file_id = "2ZKH7K7M9G3G0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let test_image_blob_name = "SynonymLogo.png";

    let test_image_dir_path = format!("static/files/{test_file_user}/{test_file_id}/");
    let full_image_path = format!("{}/main", test_image_dir_path.clone());

    // make sure directory exists
    let exists = match fs::metadata(test_image_dir_path.clone()) {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };
    if !exists {
        create_dir_all(&test_image_dir_path)?;
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

    let client = httpc_test::new_client(HOST_URL)?;
    let test_file_version_name = "main";
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

    remove_dir_all(&test_image_dir_path)?;
    Ok(())
}

#[tokio::test]
async fn test_static_image_serving_feed() -> Result<()> {
    setup(&Config::from_env()).await;
    let test_file_id = "2ZKH7K7M9G3G0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let test_image_blob_name = "SynonymLogo.png";

    let test_image_dir_path = format!("static/files/{test_file_user}/{test_file_id}/");
    let full_image_path = format!("{}/main", test_image_dir_path.clone());

    // make sure directory exists
    let exists = match fs::metadata(test_image_dir_path.clone()) {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };
    if !exists {
        create_dir_all(&test_image_dir_path)?;
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

    let client = httpc_test::new_client(HOST_URL)?;
    let test_file_version_name = "feed";
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
        "image/jpeg"
    );

    remove_dir_all(&test_image_dir_path)?;
    Ok(())
}

#[tokio::test]
async fn test_static_image_serving_small() -> Result<()> {
    setup(&Config::from_env()).await;
    let test_file_id = "2ZKH7K7M9G3G0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let test_image_blob_name = "SynonymLogo.png";

    let test_image_dir_path = format!("static/files/{test_file_user}/{test_file_id}/");
    let full_image_path = format!("{}/main", test_image_dir_path.clone());

    // make sure directory exists
    let exists = match fs::metadata(test_image_dir_path.clone()) {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };
    if !exists {
        create_dir_all(&test_image_dir_path)?;
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

    let client = httpc_test::new_client(HOST_URL)?;
    let test_file_version_name = "small";
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
        "image/jpeg"
    );

    remove_dir_all(&test_image_dir_path)?;
    Ok(())
}
