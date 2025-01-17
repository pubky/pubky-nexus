use std::{
    fs::{self, create_dir_all, remove_file, File},
    io::Write,
};

use anyhow::Result;
use serde_json::json;

const HOST_URL: &str = "http://localhost:8080";

#[tokio::test]
async fn test_static_serving() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;
    let test_file_path = "static";
    let test_file_name = "foobar";

    let full_path = format!("{}/{}", test_file_path, test_file_name);

    let exists = match fs::metadata(test_file_path) {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };

    println!("file exists? {}", exists);

    if !exists {
        create_dir_all(test_file_path)?;
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
            .parse::<i32>()
            .unwrap(),
        13
    );

    remove_file(full_path.as_str())?;
    Ok(())
}

#[tokio::test]
async fn test_file_details() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;
    let test_file_id = "2ZK2H8P2T5NG0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

    let test_file_uri = format!("pubky://{test_file_user}/pub/pubky.app/files/{test_file_id}");

    let res = client
        .do_get(
            format!(
                "/v0/files/file/{}",
                url::form_urlencoded::byte_serialize(test_file_uri.as_bytes()).collect::<String>()
            )
            .as_str(),
        )
        .await?;

    let json_body = res.json_body()?;
    assert_eq!(res.status(), 200);
    assert_eq!(json_body["id"], test_file_id);
    assert_eq!(json_body["owner_id"], test_file_user);

    Ok(())
}

#[tokio::test]
async fn test_files_by_ids() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;
    let test_file_id = "2ZK2H8P2T5NG0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let test_file_uri = format!("pubky://{test_file_user}/pub/pubky.app/files/{test_file_id}");

    let test_file_id2 = "2ZK1VCJN4YE00";
    let test_file_user2 = "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy";
    let test_file_uri2 = format!("pubky://{test_file_user2}/pub/pubky.app/files/{test_file_id2}");

    let res = client
        .do_post(
            "/v0/files/by-ids",
            json!({"uris": [test_file_uri, test_file_uri2]}),
        )
        .await?;

    let json_body = res.json_body()?;

    assert_eq!(res.status(), 200);
    assert_eq!(json_body.as_array().unwrap().len(), 2);

    assert_eq!(json_body[0]["id"], test_file_id);
    assert_eq!(json_body[0]["owner_id"], test_file_user);
    assert_eq!(json_body[1]["id"], test_file_id2);
    assert_eq!(json_body[1]["owner_id"], test_file_user2);

    Ok(())
}
