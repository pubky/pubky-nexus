use anyhow::Result;
use tokio;

#[tokio::test]
async fn test_info_endpoint() -> Result<()> {
    let client = httpc_test::new_client("http://localhost:8080")?;

    let res = client.do_get("/v0/info").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["name"], env!("CARGO_PKG_NAME"));
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));

    Ok(())
}

#[tokio::test]
async fn test_profile_endpoint() -> Result<()> {
    let client = httpc_test::new_client("http://localhost:8080")?;

    // Look for Aldert pk user id
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client.do_get(&format!("/v0/profile/{}", user_id)).await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["id"], user_id);
    assert_eq!(body["name"], "Aldert");
    assert_eq!(body["status"], "working");

    // Look for a non existing pk
    let user_id = "bad_user_id";
    let res = client.do_get(&format!("/v0/profiles/{}", user_id)).await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_static_file_serving() -> Result<()> {
    let client = httpc_test::new_client("http://localhost:8080")?;

    let res = client.do_get("/static/src/main.rs").await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("fn main()"));

    Ok(())
}

#[tokio::test]
async fn test_swagger_ui() -> Result<()> {
    let client = httpc_test::new_client("http://localhost:8080")?;

    let res = client.do_get("/swagger-ui").await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("<html"));

    Ok(())
}

#[tokio::test]
async fn test_openapi_schema() -> Result<()> {
    let client = httpc_test::new_client("http://localhost:8080")?;

    let res = client.do_get("/api-docs/openapi.json").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["openapi"].is_string());
    assert!(body["info"]["title"].is_string());
    assert_eq!(body["info"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["paths"].is_object());

    Ok(())
}

// Intended to print out requests and play around as a client while developing
#[tokio::test]
async fn quick_dev() -> Result<()> {
    let hc = httpc_test::new_client("http://localhost:8080")?;

    // Check endpoint, play with this.
    hc.do_get("/v0/info").await?.print().await?;

    Ok(())
}
