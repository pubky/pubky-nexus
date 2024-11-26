use crate::service::utils::HOST_URL;
use anyhow::Result;

#[tokio::test]
async fn test_user_endpoint() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Look for Aldert pk user id
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client.do_get(&format!("/v0/user/{}", user_id)).await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["details"]["name"], "Aldert");
    assert_eq!(body["details"]["status"], "working");
    assert_eq!(body["details"]["id"], user_id);
    assert_eq!(body["details"]["image"], "pubky://4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro/pub/pubky.app/files/003286NSMY490");
    assert_eq!(body["counts"]["friends"], 8);
    assert_eq!(body["counts"]["posts"], 4);

    // Test tags on Ar's profile
    let ar_id = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    let res = client.do_get(&format!("/v0/user/{}", ar_id)).await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    //let user_profile: UserView = serde_json::from_value(body)?;
    if let Some(tags) = body.get("tags").and_then(|t| t.as_array()) {
        assert_eq!(tags.len(), 3);
        assert!(
            tags.iter().any(|tag| tag["label"] == "pkarr"),
            "Ar profile should tagged as 'pkarr'"
        );
        assert!(
            tags.iter().any(|tag| tag["label"] == "synonym"),
            "Ar profile should tagged as 'synonym'"
        );
        assert!(
            !tags.iter().any(|tag| tag["label"] == "nonsense"),
            "Ar profile should tagged as 'nonsense'"
        );
    }

    // Look for Aldert pk user id using Flavio's viewer id
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let res = client
        .do_get(&format!("/v0/user/{}?viewer_id={}", user_id, viewer_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(
        body["relationship"]["followed_by"], true,
        "Aldert should follow Flavio"
    );
    assert_eq!(
        body["relationship"]["following"], false,
        "Flavio should not follow Aldert"
    );

    // Look for a non existing pk
    let user_id = "bad_user_id";
    let res = client.do_get(&format!("/v0/user/{}", user_id)).await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_relationship() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let res = client
        .do_get(&format!("/v0/user/{}/relationship/{}", user_id, viewer_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["following"].is_boolean());
    assert!(body["followed_by"].is_boolean());

    // Test non-existing relationship
    let user_id = "bad_user_id";
    let viewer_id = "bad_viewer_id";
    let res = client
        .do_get(&format!("/v0/user/{}/relationship/{}", user_id, viewer_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_counts() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!("/v0/user/{}/counts", user_id))
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
        .do_get(&format!("/v0/user/{}/counts", user_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_details() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!("/v0/user/{}/details", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;

    assert!(body["name"].is_string());
    assert!(body["bio"].is_string());
    assert!(body["id"].is_string());
    assert!(body["status"].is_string());
    assert!(body["links"].is_array());
    assert!(body["indexed_at"].is_number());

    // Test non-existing user
    let user_id = "bad_user_id";
    let res = client
        .do_get(&format!("/v0/user/{}/details", user_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_muted() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "db6w580d5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy";
    let res = client
        .do_get(&format!("/v0/user/{}/muted", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());
    let muted: Vec<String> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect();

    // List of specified IDs the user is expected have muted
    let specified_ids = vec![
        "rz6oe4yda9em9b4m7ymt8gym3r9g5gfa51su3rgdj9oszyz787n5".to_string(),
        "5f4e800ogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to".to_string(),
        "58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qag0".to_string(),
    ];

    // Check if the user muted the specified number of users
    assert_eq!(
        muted.len(),
        specified_ids.len(),
        "Unexpected number of muted users"
    );

    // Check if all specified IDs are present in the muted list
    for id in &specified_ids {
        assert!(muted.contains(id), "Missing muted ID: {}", id);
    }

    // Test non-existing user
    let res = client
        .do_get(&format!("/v0/user/{}/muted", "bad_user_id"))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}
