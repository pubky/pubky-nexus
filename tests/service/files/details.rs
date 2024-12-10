use anyhow::Result;

use crate::service::utils::HOST_URL;

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
