use anyhow::Result;
use serde_json::Value;

use crate::service::utils::make_request;

// TODO: Create deterministic integration tests

const PEER_PUBKY: &str = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";

struct StreamTagMockup {
    label: String,
    tagger_ids: usize,
    tagged_count: u64,
    taggers_count: usize,
}

impl StreamTagMockup {
    fn new(label: String, tagger_ids: usize, tagged_count: u64, taggers_count: usize) -> Self {
        Self {
            label,
            tagger_ids,
            tagged_count,
            taggers_count,
        }
    }
}

// Small unit test to compare all the tags composition
fn analyse_hot_tags_structure(tags: &Vec<Value>) {
    for tag in tags {
        assert!(tag["label"].is_string(), "label should be a string");
        assert!(
            tag["taggers_id"].is_array(),
            "tagger_ids should be an array"
        );
        assert!(
            tag["tagged_count"].is_number(),
            "tagged_count should be a number"
        );
        assert!(
            tag["taggers_count"].is_number(),
            "taggers_count should be a number"
        );
    }
}

// Small unit test to compare the tag properties
fn compare_unit_hot_tag(tag: &Value, hot_tag: StreamTagMockup) {
    assert_eq!(tag["tagged_count"], hot_tag.tagged_count);
    assert_eq!(tag["label"], hot_tag.label);
    assert_eq!(tag["taggers_count"], hot_tag.taggers_count);
    let tagger_ids = tag["taggers_id"].as_array().unwrap();
    assert_eq!(tagger_ids.len(), hot_tag.tagger_ids);
}

#[tokio::test]
async fn test_global_hot_tags() -> Result<()> {
    let body = make_request("/v0/tags/hot").await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 4th index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 14, 26, 14);
    compare_unit_hot_tag(&tags[3], hot_tag);

    Ok(())
}

#[tokio::test]
async fn test_global_hot_tags_for_posts() -> Result<()> {
    let body = make_request("/v0/tags/hot?tagged_type=Post").await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 4th index
    let hot_tag = StreamTagMockup::new(String::from("ha"), 9, 16, 9);
    compare_unit_hot_tag(&tags[3], hot_tag);

    Ok(())
}

#[tokio::test]
async fn test_hot_tags_by_following_reach() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot/{}/following", PEER_PUBKY,);

    let body = make_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 5, 12, 5);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio::test]
async fn test_hot_tags_by_following_max_taggers() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot/{}/following?max_taggers=3", PEER_PUBKY,);

    let body = make_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 3, 12, 5);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio::test]
async fn test_hot_tags_by_followers_reach() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot/{}/followers", PEER_PUBKY);

    let body = make_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Post stream should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 1st index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 3, 10, 3);
    compare_unit_hot_tag(&tags[1], hot_tag);

    Ok(())
}

#[tokio::test]
async fn test_hot_tags_by_friends_reach() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot/{}/friends", PEER_PUBKY);

    let body = make_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Post stream should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 1st index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 3, 10, 3);
    compare_unit_hot_tag(&tags[1], hot_tag);

    Ok(())
}
