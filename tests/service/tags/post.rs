use anyhow::Result;
use reqwest::StatusCode;

use crate::service::{
    tags::user::PUBKY_PEER,
    utils::{get_request, invalid_get_request},
};

use super::utils::{analyse_tag_details_structure, compare_tag_details, TagMockup};

// Peter user from test/tags.cypher
pub const PEER_PUBKY: &str = "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy";
const POST_ID: &str = "HC3T5CEPBPHQ";
const FREE_LABEL: &str = "free";
const ANONYMOUS_PUBKY: &str = "mwsnc3qzej8hks6motdeyj8ag7gzaf3ft5emcjzk9wn5erxg968y";

const BAHRINGER_USER: &str = "7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso";
const BAHRINGER_POST: &str = "2Z1N9M56X4EG0";

#[tokio_shared_rt::test(shared)]
async fn test_post_tag() -> Result<()> {
    let path = format!("/v0/post/{}/{}/tags", PEER_PUBKY, POST_ID);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");
    // Check the total tags of the post
    assert_eq!(tags.len(), 3);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("lg"), 4, 4);
    compare_tag_details(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_limit_tag_filter_active() -> Result<()> {
    let path = format!("/v0/post/{}/{}/tags?limit_tags=2", PEER_PUBKY, POST_ID);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 2);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 2nd index
    let hot_tag = TagMockup::new(String::from("free"), 3, 3);
    compare_tag_details(&tags[1], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_viewer_filter_active() -> Result<()> {
    let path = format!(
        "/v0/post/{}/{}/tags?viewer_id={}",
        PEER_PUBKY, POST_ID, PUBKY_PEER
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 3);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    assert!(
        tags[0]["is_tagger"].as_bool().unwrap(),
        "Expected to be part of the taggers"
    );
    assert!(
        !tags[1]["is_tagger"].as_bool().unwrap(),
        "Expected not to be part of the taggers"
    );
    assert!(
        tags[2]["is_tagger"].as_bool().unwrap(),
        "Expected to be part of the taggers"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_skip_tag_filter_active() -> Result<()> {
    let path = format!(
        "/v0/post/{}/{}/tags?skip_tags=7",
        BAHRINGER_USER, BAHRINGER_POST
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 3);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // Analyse the tag that is in the 1st index
    let hot_tag = TagMockup::new(String::from("emergent"), 1, 1);
    compare_tag_details(&tags[0], hot_tag);
    // Analyse the tag that is in the 3rd index
    let hot_tag = TagMockup::new(String::from("cheap"), 1, 1);
    compare_tag_details(&tags[2], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_skip_and_limit_tag_filter_active() -> Result<()> {
    let path = format!(
        "/v0/post/{}/{}/tags?skip_tags=4&limit_tags=3",
        BAHRINGER_USER, BAHRINGER_POST
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 3);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // Analyse the tag that is in the 1st index
    let hot_tag = TagMockup::new(String::from("mutation"), 1, 1);
    compare_tag_details(&tags[0], hot_tag);
    // Analyse the tag that is in the 2nd index
    let hot_tag = TagMockup::new(String::from("irritably"), 1, 1);
    compare_tag_details(&tags[1], hot_tag);
    // Analyse the tag that is in the 3rd index
    let hot_tag = TagMockup::new(String::from("frantically"), 1, 1);
    compare_tag_details(&tags[2], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_limit_taggers_filter_active() -> Result<()> {
    let path = format!("/v0/post/{}/{}/tags?limit_taggers=1", PEER_PUBKY, POST_ID);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 3);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("free"), 1, 3);
    compare_tag_details(&tags[1], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_tags_full_filter_active() -> Result<()> {
    let path = format!(
        "/v0/post/{}/{}/tags?limit_tags=1&limit_taggers=1",
        PEER_PUBKY, POST_ID
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 1);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("lg"), 1, 4);
    compare_tag_details(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_does_not_exist() -> Result<()> {
    let endpoint = format!("/v0/post/{}/{}/tags", PEER_PUBKY, "JTDX9ZSWPQF8");
    // TODO: Control post not found error control
    invalid_get_request(&endpoint, StatusCode::NOT_FOUND).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_does_not_exist() -> Result<()> {
    let endpoint = format!(
        "/v0/post/{}/{}/tags",
        "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgc4", POST_ID
    );
    // TODO: Control post not found error control
    invalid_get_request(&endpoint, StatusCode::NOT_FOUND).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag() -> Result<()> {
    let path = format!("/v0/post/{}/{}/taggers/{}", PEER_PUBKY, POST_ID, FREE_LABEL);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let taggers = body.as_array().expect("Tag list should be an array");
    let taggers_list = taggers[0].as_array().expect("Tag list should be an array");
    assert_eq!(taggers_list.len(), 3);

    assert_eq!(
        &taggers_list[2],
        "58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qagy"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag_with_limit() -> Result<()> {
    let path = format!(
        "/v0/post/{}/{}/taggers/{}?limit=1",
        PEER_PUBKY, POST_ID, FREE_LABEL
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let taggers = body.as_array().expect("Tag list should be an array");
    let taggers_list = taggers[0].as_array().expect("Tag list should be an array");
    assert_eq!(taggers_list.len(), 1);

    assert_eq!(&taggers_list[0], ANONYMOUS_PUBKY);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag_with_viewer_id() -> Result<()> {
    let path = format!(
        "/v0/post/{}/{}/taggers/{}?viewer_id={}",
        PEER_PUBKY, POST_ID, FREE_LABEL, ANONYMOUS_PUBKY
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let taggers = body.as_array().expect("Tag list should be an array");
    let taggers_list = taggers[0].as_array().expect("Tag list should be an array");
    assert_eq!(taggers_list.len(), 3);

    assert!(taggers[1].as_bool().unwrap());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag_with_skip() -> Result<()> {
    let path = format!(
        "/v0/post/{}/{}/taggers/{}?skip=1",
        PEER_PUBKY, POST_ID, FREE_LABEL
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let taggers = body.as_array().expect("Tag list should be an array");
    let taggers_list = taggers[0].as_array().expect("Tag list should be an array");
    assert_eq!(taggers_list.len(), 2);

    assert_eq!(
        &taggers_list[0],
        "rz6oe4yda9em9b4m7ymttgym3r9g5gfa51su3rgdj9oszyz787ny"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag_with_full_filters() -> Result<()> {
    let path = format!(
        "/v0/post/{}/{}/taggers/{}?skip=2&limit=1",
        PEER_PUBKY, POST_ID, FREE_LABEL
    );
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let taggers = body.as_array().expect("Tag list should be an array");
    let taggers_list = taggers[0].as_array().expect("Tag list should be an array");
    assert_eq!(taggers_list.len(), 1);

    assert_eq!(
        &taggers_list[0],
        "58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qagy"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag_with_no_result() -> Result<()> {
    let path = format!(
        "/v0/post/{}/{}/taggers/{}?skip=3&limit=1",
        PEER_PUBKY, POST_ID, FREE_LABEL
    );
    invalid_get_request(&path, StatusCode::NOT_FOUND).await?;

    Ok(())
}

// TODO: Check if it is in the cache
