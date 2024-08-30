use serde_json::Value;

use crate::service::tags::HOST_URL;

// #######################################
// ##### Endpoint requests related #######
// #######################################

// Small unit test to test the endpoint
pub async fn make_request(endpoint: &str) -> Result<Value, httpc_test::Error> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get(endpoint).await?;

    assert_eq!(res.status(), 200);
    let body = res.json_body()?;
    Ok(body)
}

// ################################
// ##### TagDetails related #######
// ################################

pub struct TagMockup {
    pub label: String,
    pub taggers: usize,
    pub taggers_count: usize,
}

impl TagMockup {
    pub fn new(label: String, taggers: usize, taggers_count: usize) -> Self {
        Self {
            label,
            taggers,
            taggers_count,
        }
    }
}

// Small unit test to compare all the tag details composition
pub fn analyse_tag_details_structure(tags: &Vec<Value>) {
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
pub fn compare_tag_details(tag: &Value, hot_tag: TagMockup) {
    assert_eq!(tag["label"], hot_tag.label);
    let tagger_ids = tag["taggers"].as_array().unwrap();
    assert_eq!(tagger_ids.len(), hot_tag.taggers);
    assert_eq!(tag["taggers_count"], hot_tag.taggers_count);
}
