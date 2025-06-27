use crate::{
    tags::user::PUBKY_PEER,
    utils::{get_request, invalid_get_request},
};
use anyhow::Result;
use axum::http::StatusCode;

#[tokio_shared_rt::test(shared)]
async fn test_user_endpoint() -> Result<()> {
    // Look for Aldert pk user id
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = get_request(&format!("/v0/user/{user_id}")).await?;

    assert_eq!(res["details"]["name"], "Aldert");
    assert_eq!(res["details"]["status"], "working");
    assert_eq!(res["details"]["id"], user_id);
    assert_eq!(res["details"]["image"], "pubky://4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro/pub/pubky.app/files/003286NSMY490");

    assert_eq!(res["counts"]["tagged"], 10);
    assert_eq!(res["counts"]["tags"], 4);
    assert_eq!(res["counts"]["unique_tags"], 4);
    assert_eq!(res["counts"]["posts"], 4);
    assert_eq!(res["counts"]["replies"], 0);
    assert_eq!(res["counts"]["following"], 15);
    assert_eq!(res["counts"]["followers"], 10);
    assert_eq!(res["counts"]["friends"], 8);
    assert_eq!(res["counts"]["bookmarks"], 0);

    // Test tags on Ar's profile
    let ar_id = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    let res = get_request(&format!("/v0/user/{ar_id}")).await?;

    //let user_profile: UserView = serde_json::from_value(body)?;
    if let Some(tags) = res.get("tags").and_then(|t| t.as_array()) {
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
    let res = get_request(&format!("/v0/user/{user_id}?viewer_id={viewer_id}")).await?;

    assert_eq!(
        res["relationship"]["followed_by"], true,
        "Aldert should follow Flavio"
    );
    assert_eq!(
        res["relationship"]["following"], false,
        "Flavio should not follow Aldert"
    );

    // Look for a non existing pk
    let user_id = "bad_user_id";
    invalid_get_request(&format!("/v0/user/{user_id}"), StatusCode::NOT_FOUND).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_relationship() -> Result<()> {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let res = get_request(&format!("/v0/user/{user_id}/relationship/{viewer_id}")).await?;

    assert!(res["following"].is_boolean());
    assert!(res["followed_by"].is_boolean());

    // Test non-existing relationship
    let user_id = "bad_user_id";
    let viewer_id = "bad_viewer_id";
    invalid_get_request(
        &format!("/v0/user/{user_id}/relationship/{viewer_id}"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_view_tags() -> Result<()> {
    let user_id = PUBKY_PEER;
    let viewer_id = "58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qagy";
    let res = get_request(&format!("/v0/user/{user_id}?viewer_id={viewer_id}")).await?;

    assert!(res["tags"][0]["relationship"].as_bool().unwrap());
    assert!(res["tags"][1]["relationship"].as_bool().unwrap());
    assert!(!res["tags"][2]["relationship"].as_bool().unwrap());
    assert!(!res["tags"][3]["relationship"].as_bool().unwrap());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_counts() -> Result<()> {
    let user_id = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    let res = get_request(&format!("/v0/user/{user_id}/counts")).await?;

    assert!(res["tagged"].is_number());
    assert_eq!(res["tagged"], 95);
    assert!(res["tags"].is_number());
    assert_eq!(res["tags"], 7);
    assert!(res["unique_tags"].is_number());
    assert_eq!(res["unique_tags"], 3);
    assert!(res["replies"].is_number());
    assert_eq!(res["replies"], 23);
    assert!(res["posts"].is_number());
    assert_eq!(res["posts"], 87);
    assert!(res["followers"].is_number());
    assert_eq!(res["followers"], 27);
    assert!(res["following"].is_number());
    assert_eq!(res["following"], 7);
    assert!(res["friends"].is_number());
    assert_eq!(res["friends"], 6);
    assert!(res["bookmarks"].is_number());
    assert_eq!(res["bookmarks"], 14);

    // Test non-existing user
    let user_id = "bad_user_id";
    invalid_get_request(&format!("/v0/user/{user_id}/counts"), StatusCode::NOT_FOUND).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_details() -> Result<()> {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = get_request(&format!("/v0/user/{user_id}/details")).await?;

    assert!(res["name"].is_string());
    assert!(res["bio"].is_string());
    assert!(res["id"].is_string());
    assert!(res["status"].is_string());
    assert!(res["links"].is_array());
    assert!(res["indexed_at"].is_number());

    // Test non-existing user
    let user_id = "bad_user_id";
    invalid_get_request(
        &format!("/v0/user/{user_id}/details"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_muted() -> Result<()> {
    let user_id = "db6w580d5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy";
    let res = get_request(&format!("/v0/user/{user_id}/muted")).await?;

    assert!(res.is_array());
    let muted: Vec<String> = res
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
        assert!(muted.contains(id), "Missing muted ID: {id}");
    }

    // Test non-existing user
    invalid_get_request(
        &format!("/v0/user/{}/muted", "bad_user_id"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}
