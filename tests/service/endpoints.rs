use anyhow::Result;

use crate::utils::TestServiceServer;

const HOST_URL: &str = "http://localhost:8080";

#[tokio_shared_rt::test(shared)]
async fn test_swagger_ui() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/swagger-ui").await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("<html"));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_openapi_schema() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/api-docs/openapi.json").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["openapi"].is_string());
    assert!(body["info"]["title"].is_string());
    assert_eq!(body["info"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["paths"].is_object());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_info_endpoint() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/v0/info").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["name"], env!("CARGO_PKG_NAME"));
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));

    Ok(())
}
