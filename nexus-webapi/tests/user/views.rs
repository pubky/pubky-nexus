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
    let user_id = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
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
    let user_id = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
    let viewer_id = "q5ef4o3jqxnpadzuk1h3qgqi1kefwf6zs7yuofmr6ynymt593ieo";
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
    assert!(res["collections"].is_number());
    assert_eq!(res["collections"], 0, "user authored no collections");

    // Test non-existing user
    let user_id = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
    invalid_get_request(&format!("/v0/user/{user_id}/counts"), StatusCode::NOT_FOUND).await?;

    Ok(())
}

/// Counts derived from the seed in `docker/test-graph/mocks/posts.cypher`.
#[tokio_shared_rt::test(shared)]
async fn test_user_counts_collections_and_bookmark_exclusion() -> Result<()> {
    let bogota = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";
    let res = get_request(&format!("/v0/user/{bogota}/counts")).await?;
    assert_eq!(res["collections"], 4, "Bogota authored 4 collections");

    let cairo = "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o";
    let res = get_request(&format!("/v0/user/{cairo}/counts")).await?;
    assert_eq!(res["collections"], 1, "Cairo authored 1 collection");

    // Eixample bookmarks one normal post and one collection.
    let eixample = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";
    let res = get_request(&format!("/v0/user/{eixample}/counts")).await?;
    assert_eq!(
        res["bookmarks"], 1,
        "collection-follow excluded from bookmarks"
    );
    assert_eq!(res["collections"], 0, "Eixample authored no collections");

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
    let user_id = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
    invalid_get_request(
        &format!("/v0/user/{user_id}/details"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}
