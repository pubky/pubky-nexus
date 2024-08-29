use anyhow::Result;
use serde_json::Value;

use super::{make_request, TagMockup};

// Arst user from test.cypher
const PUBKY_PEER: &str = "5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to";

// Small unit test to compare all the tags composition
fn analyse_hot_tags_structure(tags: &Vec<Value>) {
    for tag in tags {
        assert!(tag["label"].is_string(), "label should be a string");
        assert!(tag["taggers"].is_array(), "taggers should be an array");
        assert!(
            tag["taggers_count"].is_number(),
            "taggers_count should be a number"
        );
    }
}

// Small unit test to compare the tag properties
fn compare_unit_hot_tag(tag: &Value, hot_tag: TagMockup) {
    assert_eq!(tag["label"], hot_tag.label);
    let tagger_ids = tag["taggers"].as_array().unwrap();
    assert_eq!(tagger_ids.len(), hot_tag.taggers);
    assert_eq!(tag["taggers_count"], hot_tag.taggers_count);
}

#[tokio::test]
async fn test_full_user_tags_endpoint() -> Result<()> {
    let path = format!("/v0/user/{}/tags", PUBKY_PEER);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Tag list should be an array");
    assert_eq!(tags.len(), 4);

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("privacy"), 2, 2);
    compare_unit_hot_tag(&tags[1], hot_tag);

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
    analyse_hot_tags_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("pubky"), 3, 3);
    compare_unit_hot_tag(&tags[0], hot_tag);

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
    analyse_hot_tags_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("hike"), 1, 2);
    compare_unit_hot_tag(&tags[2], hot_tag);

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
    analyse_hot_tags_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("pubky"), 1, 3);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}
// TODO: Check if it is in the cache. Maybe we should add under tests/service: endpoints, db, ...
