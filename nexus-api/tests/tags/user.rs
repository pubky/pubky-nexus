use anyhow::Result;
use nexus_common::types::routes::TaggersInfo;
use reqwest::StatusCode;

use crate::{
    tags::PEER_PUBKY,
    utils::{get_request, invalid_get_request},
};

use super::utils::{analyse_tag_details_structure, compare_tag_details, TagMockup};

// Arst user from test/tags.cypher
pub const PUBKY_PEER: &str = "5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to";
const PUBKY_LABEL: &str = "pubky";

#[tokio_shared_rt::test(shared)]
async fn test_full_user_tags_endpoint() -> Result<()> {
    let path = format!("/v0/user/{}/tags", PUBKY_PEER);
    let body = get_request(&path).await?;

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
#[tokio_shared_rt::test(shared)]
async fn test_user_tags_with_viewer_id() -> Result<()> {
    let path = format!(
        "/v0/user/{}/tags?viewer_id=rz6oe4yda9em9b4m7ymttgym3r9g5gfa51su3rgdj9oszyz787ny",
        PUBKY_PEER
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 4);

    assert!(
        tags[0]["relationship"].as_bool().unwrap(),
        "Expected to be part of the taggers"
    );
    assert!(
        !tags[1]["relationship"].as_bool().unwrap(),
        "Expected not to be part of the taggers"
    );
    assert!(
        !tags[2]["relationship"].as_bool().unwrap(),
        "Expected to be part of the taggers"
    );
    assert!(
        tags[3]["relationship"].as_bool().unwrap(),
        "Expected to be part of the taggers"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_limit_tag_filter_active() -> Result<()> {
    let path = format!("/v0/user/{}/tags?limit_tags=2", PUBKY_PEER);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 2);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 1st index
    let hot_tag = TagMockup::new(String::from("pubky"), 3, 3);
    compare_tag_details(&tags[0], hot_tag);

    Ok(())
}

const MEDHURST_USER: &str = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy";
const RECKLESSLY_TAG: &str = "recklessly";
const EVEN_TAG: &str = "even";
const WEBBED_TAG: &str = "webbed";

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_skip_tag_filter_active() -> Result<()> {
    let path = format!("/v0/user/{}/tags?skip_tags=1", MEDHURST_USER);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 5);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let tag = TagMockup::new(String::from(EVEN_TAG), 2, 2);
    compare_tag_details(&tags[1], tag);

    let tag = TagMockup::new(String::from(RECKLESSLY_TAG), 1, 1);
    compare_tag_details(&tags[4], tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_skip_and_limit_tag_filter_active() -> Result<()> {
    let path = format!("/v0/user/{}/tags?skip_tags=5&limit_tags=1", MEDHURST_USER);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 1);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let tag = TagMockup::new(String::from(RECKLESSLY_TAG), 1, 1);
    compare_tag_details(&tags[0], tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_skip_limit_and_taggers_limit_filter_active() -> Result<()> {
    let path = format!(
        "/v0/user/{}/tags?skip_tags=2&limit_tags=2&limit_taggers=1",
        MEDHURST_USER
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 2);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let tag = TagMockup::new(String::from(EVEN_TAG), 1, 2);
    compare_tag_details(&tags[0], tag);

    let tag = TagMockup::new(String::from(WEBBED_TAG), 1, 1);
    compare_tag_details(&tags[1], tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_limit_taggers_filter_active() -> Result<()> {
    let path = format!("/v0/user/{}/tags?limit_taggers=1", PUBKY_PEER);
    let body = get_request(&path).await?;

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

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_full_filter_active() -> Result<()> {
    let path = format!("/v0/user/{}/tags?limit_tags=1&limit_taggers=1", PUBKY_PEER);
    let body = get_request(&path).await?;

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

#[tokio_shared_rt::test(shared)]
async fn test_user_does_not_exist() -> Result<()> {
    let endpoint = format!(
        "/v0/user/{}/tags",
        "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgc4"
    );
    // TODO: Control post not found error control
    invalid_get_request(&endpoint, StatusCode::NOT_FOUND).await?;
    Ok(())
}

// #### USER TAGGERS ######

#[tokio_shared_rt::test(shared)]
async fn test_user_specific_tag() -> Result<()> {
    let path = format!("/v0/user/{}/taggers/{}", PUBKY_PEER, PUBKY_LABEL);
    let body = get_request(&path).await?;

    let taggers_info: TaggersInfo = serde_json::from_value(body)?;

    assert_eq!(taggers_info.users.len(), 3);

    assert_eq!(
        &taggers_info.users[2],
        "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_specific_tag_with_viewer_id() -> Result<()> {
    let path = format!(
        "/v0/user/{}/taggers/{}?viewer_id={}",
        PUBKY_PEER, PUBKY_LABEL, PEER_PUBKY
    );
    let body = get_request(&path).await?;

    let taggers_info: TaggersInfo = serde_json::from_value(body)?;
    assert_eq!(taggers_info.users.len(), 3);

    assert_eq!(
        &taggers_info.users[2],
        "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_specific_tag_with_limit() -> Result<()> {
    let path = format!("/v0/user/{}/taggers/{}?limit=1", PUBKY_PEER, PUBKY_LABEL);
    let body = get_request(&path).await?;

    let taggers_info: TaggersInfo = serde_json::from_value(body)?;

    assert_eq!(taggers_info.users.len(), 1);

    assert_eq!(
        &taggers_info.users[0],
        "58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qagy"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_specific_tag_with_skip() -> Result<()> {
    let path = format!("/v0/user/{}/taggers/{}?skip=1", PUBKY_PEER, PUBKY_LABEL);
    let body = get_request(&path).await?;

    let taggers_info: TaggersInfo = serde_json::from_value(body)?;

    assert_eq!(taggers_info.users.len(), 2);

    assert_eq!(
        &taggers_info.users[0],
        "rz6oe4yda9em9b4m7ymttgym3r9g5gfa51su3rgdj9oszyz787ny"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_specific_tag_with_full_filters() -> Result<()> {
    let path = format!(
        "/v0/user/{}/taggers/{}?skip=2&limit=1",
        PUBKY_PEER, PUBKY_LABEL
    );
    let body = get_request(&path).await?;

    let taggers_info: TaggersInfo = serde_json::from_value(body)?;

    assert_eq!(taggers_info.users.len(), 1);

    assert_eq!(
        &taggers_info.users[0],
        "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_specific_tag_with_no_result() -> Result<()> {
    let path = format!(
        "/v0/user/{}/taggers/{}?skip=3&limit=1",
        PUBKY_PEER, PUBKY_LABEL
    );
    invalid_get_request(&path, StatusCode::NOT_FOUND).await?;

    Ok(())
}
// TODO: Check if it is in the cache. Maybe we should add under tests/service: endpoints, db, ...
