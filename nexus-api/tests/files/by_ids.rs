use anyhow::Result;
use serde_json::json;

use crate::utils::post_request;

#[tokio_shared_rt::test(shared)]
async fn test_files_by_ids() -> Result<()> {
    let test_file_id = "2ZK2H8P2T5NG0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let test_file_uri = format!("pubky://{test_file_user}/pub/pubky.app/files/{test_file_id}");

    let test_file_id2 = "2ZK1VCJN4YE00";
    let test_file_user2 = "sfgetccnq7s3h57a7imf6n7k5fqxus33yg85f1ndhnrnofjdmhjy";
    let test_file_uri2 = format!("pubky://{test_file_user2}/pub/pubky.app/files/{test_file_id2}");

    let json_body = post_request(
        "/v0/files/by_ids",
        json!({"uris": [test_file_uri, test_file_uri2]}),
    )
    .await?;

    assert_eq!(json_body.as_array().unwrap().len(), 2);

    assert_eq!(json_body[0]["id"], test_file_id);
    assert_eq!(json_body[0]["owner_id"], test_file_user);
    assert_eq!(json_body[1]["id"], test_file_id2);
    assert_eq!(json_body[1]["owner_id"], test_file_user2);

    Ok(())
}
