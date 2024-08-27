use anyhow::Result;
use serde_json::Value;

use crate::tags::make_request;

const PEER_PUBKY: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const POST_ID: &str = "2Z9P8AN738C00";

struct TagPostMockup {
    label: String,
    taggers: usize,
}

impl TagPostMockup {
    fn new(label: String, taggers: usize) -> Self {
        Self {
            label,
            taggers,
        }
    }
}

// Small unit test to compare all the tags composition
fn analyse_hot_tags_structure(tags: &Vec<Value>) {
    for tag in tags {
        assert!(tag["label"].is_string(), "label should be a string");
        assert!(
            tag["taggers"].is_array(),
            "taggers should be an array"
        );
    }
}

// Small unit test to compare the tag properties
fn compare_unit_hot_tag(tag: &Value, hot_tag: TagPostMockup) {
    assert_eq!(tag["label"], hot_tag.label);
    let tagger_ids = tag["taggers"].as_array().unwrap();
    assert_eq!(tagger_ids.len(), hot_tag.taggers);
}

#[tokio::test]
async fn test_post_tag() -> Result<()> {
    let path = format!("/v0/post/{}/{}/tags", PEER_PUBKY, POST_ID);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagPostMockup::new(String::from("pubky"), 1);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

// TODO: Try with other user that has more tags
// TODO: Check if it is in the cache
