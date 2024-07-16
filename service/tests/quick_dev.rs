#![allow(unused)] // For beginning only.

use anyhow::Result;

#[tokio::test]
async fn quick_dev() -> Result<()> {
    let hc = httpc_test::new_client("http://localhost:8080")?;

    // Check OpenAPI schema and Swagger UI
    hc.do_get("/swagger-ui").await?.print().await?;
    hc.do_get("/api-docs/openapi.json").await?.print().await?;

    // Check Info endpoint
    hc.do_get("/v0/info").await?.print().await?;

    // Check get static file
    hc.do_get("/static/src/main.rs").await?.print().await?;

    Ok(())
}
