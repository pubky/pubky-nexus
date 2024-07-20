use anyhow::Result;

const HOST_URL: &str = "http://localhost:8080";

#[tokio::test]
async fn test_info_endpoint() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/v0/info").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["name"], env!("CARGO_PKG_NAME"));
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));

    Ok(())
}

#[tokio::test]
async fn test_profile_endpoint() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Look for Aldert pk user id
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client.do_get(&format!("/v0/profile/{}", user_id)).await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["details"]["name"], "Aldert");
    assert_eq!(body["details"]["status"], "working");
    assert_eq!(body["details"]["id"], user_id);
    assert_eq!(body["counts"]["friends"], 8);
    assert_eq!(body["counts"]["posts"], 4);

    // Look for Aldert pk user id using Flavio's viewer id
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let res = client
        .do_get(&format!("/v0/profile/{}?viewer_id={}", user_id, viewer_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["viewer"]["followed_by"], true); // Aldert follows Flavio
    assert_eq!(body["viewer"]["following"], false); // Flavio does not follow Alder

    // Look for a non existing pk
    let user_id = "bad_user_id";
    let res = client.do_get(&format!("/v0/profile/{}", user_id)).await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_static_file_serving() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/static/src/main.rs").await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("fn main()"));

    Ok(())
}

#[tokio::test]
async fn test_swagger_ui() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/swagger-ui").await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("<html"));

    Ok(())
}

#[tokio::test]
async fn test_openapi_schema() -> Result<()> {
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

#[tokio::test]
async fn test_get_relationship() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let res = client
        .do_get(&format!(
            "/v0/profile/{}/relationship/{}",
            user_id, viewer_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["following"].is_boolean());
    assert!(body["followed_by"].is_boolean());

    // Test non-existing relationship
    let user_id = "bad_user_id";
    let viewer_id = "bad_viewer_id";
    let res = client
        .do_get(&format!(
            "/v0/profile/{}/relationship/{}",
            user_id, viewer_id
        ))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_counts() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!("/v0/profile/{}/counts", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["tags"].is_number());
    assert!(body["posts"].is_number());
    assert!(body["followers"].is_number());
    assert!(body["following"].is_number());
    assert!(body["friends"].is_number());

    // Test non-existing user
    let user_id = "bad_user_id";
    let res = client
        .do_get(&format!("/v0/profile/{}/counts", user_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_details() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!("/v0/profile/{}/details", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["name"].is_string());
    assert!(body["bio"].is_string());
    assert!(body["id"].is_string());
    assert!(body["image"].is_string());
    assert!(body["status"].is_string());
    assert!(body["links"].is_array());

    // Test non-existing user
    let user_id = "bad_user_id";
    let res = client
        .do_get(&format!("/v0/profile/{}/details", user_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

// #[tokio::test]
// async fn test_get_tags() -> Result<()> {
//     let client = httpc_test::new_client(HOST_URL)?;

//     let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
//     let res = client
//         .do_get(&format!("/v0/profile/{}/tags", user_id))
//         .await?;
//     assert_eq!(res.status(), 200);

//     let body = res.json_body()?;
//     assert!(body["tags"].is_array());

//     // Test non-existing user
//     let user_id = "bad_user_id";
//     let res = client
//         .do_get(&format!("/v0/profile/{}/tags", user_id))
//         .await?;
//     assert_eq!(res.status(), 404);

//     Ok(())
// }

// Intended to print out requests and play around as a client while developing
#[tokio::test]
async fn quick_dev() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Check endpoint, play with this.
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    client
        .do_get(&format!("/v0/profile/{}?viewer_id={}", user_id, viewer_id))
        .await?
        .print()
        .await?;

    Ok(())
}
