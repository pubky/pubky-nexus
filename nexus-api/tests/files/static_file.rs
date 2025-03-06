use std::{
    fs::{self, File},
    io::Write,
};

use anyhow::Result;
use nexus_common::models::{file::FileDetails, traits::Collection};
use tokio::fs::create_dir_all;

use crate::utils::host_url;

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

    let test_file_path = format!("static/files/{test_file_user}/{test_file_id}");
    let test_file_name = "main";

    let full_path = format!("{}/{}", test_file_path.clone(), test_file_name);

    let exists = match fs::metadata(test_file_path.clone()) {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };

    if !exists {
        create_dir_all(test_file_path.clone()).await?;
    }

    let mut file = File::create(full_path.as_str())?;
    file.write_all(b"Hello, world!")?;

    let res = client
        .do_get(format!("/{}", full_path.as_str()).as_str())
        .await?;

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

    let test_file_path = format!("static/files/{test_file_user}/{test_file_id}");
    let test_file_name = "main";

    let full_path = format!("{}/{}", test_file_path.clone(), test_file_name);

    let exists = match fs::metadata(test_file_path.clone()) {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };

    if !exists {
        create_dir_all(test_file_path.clone()).await?;
    }

    let mut file = File::create(full_path.as_str())?;
    file.write_all(b"Hello, world!")?;

    let res = client
        .do_get(format!("/{}?dl", full_path.as_str()).as_str())
        .await?;

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
