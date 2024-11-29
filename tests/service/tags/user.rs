use anyhow::Result;

use crate::service::utils::{make_request, make_wrong_request};

use super::utils::{analyse_tag_details_structure, compare_tag_details, TagMockup};

// Arst user from test/tags.cypher
const PUBKY_PEER: &str = "5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to";
const PUBKY_LABEL: &str = "pubky";

#[tokio::test]
async fn test_full_user_tags_endpoint() -> Result<()> {
    let path = format!("/v0/user/{}/tags", PUBKY_PEER);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 4);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("privacy"), 2, 2);
    compare_tag_details(&tags[1], hot_tag);

    Ok(())
}

#[tokio::test]
async fn test_user_tags_limit_tag_filter_active() -> Result<()> {
    let path = format!("/v0/user/{}/tags?limit_tags=2", PUBKY_PEER);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 2);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("pubky"), 3, 3);
    compare_tag_details(&tags[0], hot_tag);

    Ok(())
}

#[tokio::test]
async fn test_user_tags_limit_taggers_filter_active() -> Result<()> {
    let path = format!("/v0/user/{}/tags?limit_taggers=1", PUBKY_PEER);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 4);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("hike"), 1, 2);
    compare_tag_details(&tags[2], hot_tag);

    Ok(())
}

#[tokio::test]
async fn test_user_tags_full_filter_active() -> Result<()> {
    let path = format!("/v0/user/{}/tags?limit_tags=1&limit_taggers=1", PUBKY_PEER);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 1);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("pubky"), 1, 3);
    compare_tag_details(&tags[0], hot_tag);

    Ok(())
}

#[tokio::test]
async fn test_user_does_not_exist() -> Result<()> {
    let endpoint = format!(
        "/v0/user/{}/tags",
        "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgc4"
    );
    // TODO: Control post not found error control
    make_wrong_request(&endpoint, None).await?;
    Ok(())
}

// #### USER TAGGERS ######

#[tokio::test]
async fn test_user_specific_tag() -> Result<()> {
    let path = format!("/v0/user/{}/taggers/{}", PUBKY_PEER, PUBKY_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 3);

    assert_eq!(
        &tags[2],
        "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy"
    );

    Ok(())
}

#[tokio::test]
async fn test_user_specific_tag_with_limit() -> Result<()> {
    let path = format!("/v0/user/{}/taggers/{}?limit=1", PUBKY_PEER, PUBKY_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 1);

    assert_eq!(
        &tags[0],
        "58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qagy"
    );

    Ok(())
}

#[tokio::test]
async fn test_user_specific_tag_with_skip() -> Result<()> {
    let path = format!("/v0/user/{}/taggers/{}?skip=1", PUBKY_PEER, PUBKY_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 2);

    assert_eq!(
        &tags[0],
        "rz6oe4yda9em9b4m7ymttgym3r9g5gfa51su3rgdj9oszyz787ny"
    );

    Ok(())
}

#[tokio::test]
async fn test_user_specific_tag_with_full_filters() -> Result<()> {
    let path = format!(
        "/v0/user/{}/taggers/{}?skip=2&limit=1",
        PUBKY_PEER, PUBKY_LABEL
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 1);

    assert_eq!(
        &tags[0],
        "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy"
    );

    Ok(())
}

#[tokio::test]
async fn test_user_specific_tag_with_no_result() -> Result<()> {
    let path = format!(
        "/v0/user/{}/taggers/{}?skip=3&limit=1",
        PUBKY_PEER, PUBKY_LABEL
    );
    make_wrong_request(&path, None).await?;

    Ok(())
}
// TODO: Check if it is in the cache. Maybe we should add under tests/service: endpoints, db, ...
