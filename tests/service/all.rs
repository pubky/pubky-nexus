use std::{
    fs::{self, create_dir_all, remove_file, File},
    io::Write,
};

use anyhow::Result;
use pubky_nexus::models::tag::TagDetails;
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

#[tokio::test]
async fn test_get_post() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";

    let res = client
        .do_get(&format!(
            "/v0/post/{}/{}?viewer_id={}",
            author_id, post_id, author_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;

    assert_eq!(body["details"]["content"], "I am told we can reply now!");
    assert_eq!(body["details"]["indexed_at"].as_u64(), Some(1718616844478));
    assert_eq!(body["details"]["id"], post_id);
    assert_eq!(body["details"]["author"], author_id);
    assert_eq!(body["details"]["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(
        (body["details"]["attachments"].as_array().unwrap())[0],
        "pubky://y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pub/pubky.app/files/2ZKH7K7M9G3G0".to_string()
    );
    assert_eq!(
        body["details"]["uri"],
        "pubky://y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pub/pubky.app/posts/2ZCW1TGR5BKG0"
    );
    assert_eq!(body["counts"]["tags"].as_u64(), Some(5));
    assert_eq!(body["counts"]["replies"].as_u64(), Some(2));
    assert_eq!(body["counts"]["reposts"].as_u64(), Some(1));
    assert_eq!(body["bookmark"]["indexed_at"].as_u64(), Some(1721764200000));
    assert_eq!(body["bookmark"]["id"], "2Z9PFGC3WWWW0");

    // Panic if tags vector is bigger that 1
    let post_tag_object = body["tags"][0].clone();
    let post_tag: TagDetails = serde_json::from_value(post_tag_object.clone())?;
    assert_eq!(post_tag.label, "pubky");

    // Test non-existing post
    let res = client
        .do_get(&format!("/v0/post/{}/{}", author_id, "no_post"))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}
