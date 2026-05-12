use std::{fs::File, io::Write};

use crate::utils::host_url;
use crate::utils::server::TestServiceServer;
use anyhow::Result;
use nexus_common::models::{file::FileDetails, traits::Collection};
use tokio::fs::create_dir_all;

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
    let res = client
        .do_get(format!("/{}", url_path.as_str()).as_str())
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
    let res = client
        .do_get(format!("/{}?dl", url_path.as_str()).as_str())
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
