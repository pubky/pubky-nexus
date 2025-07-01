use anyhow::Result;

use crate::utils::get_request;

#[tokio_shared_rt::test(shared)]
async fn test_file_details() -> Result<()> {
    let test_file_id = "2ZK2H8P2T5NG0";
    let test_file_user = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

    let test_file_uri = format!("pubky://{test_file_user}/pub/pubky.app/files/{test_file_id}");

    let json_body = get_request(
        format!(
            "/v0/files/file/{}",
            url::form_urlencoded::byte_serialize(test_file_uri.as_bytes()).collect::<String>()
        )
        .as_str(),
    )
    .await?;

    assert_eq!(json_body["id"], test_file_id);
    assert_eq!(json_body["owner_id"], test_file_user);

    Ok(())
}
