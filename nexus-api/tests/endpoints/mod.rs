use crate::utils::host_url;
use anyhow::Result;

#[tokio_shared_rt::test(shared)]
async fn test_swagger_ui() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let res = client.do_get("/swagger-ui").await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("<html"));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_openapi_schema() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    let res = client.do_get("/api-docs/v0/openapi.json").await?;
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
    let client = httpc_test::new_client(host_url().await)?;

    let res = client.do_get("/v0/info").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    println!("body: {body:?}");
    assert_eq!(body["name"], env!("CARGO_PKG_NAME"));
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));

    Ok(())
}
