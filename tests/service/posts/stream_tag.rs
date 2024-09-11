use anyhow::Result;
use serde_json::Value;

use crate::service::utils::{make_request, make_wrong_request};

const ROOT_PATH: &str = "/v0/stream/posts/tag";

const OPENSOURCE_LABEL: &str = "opensource";
// Peter user from test/tags.cypher
const VIEWER_ID: &str = "f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a";

const POST_A: &str = "V8N1P3L9J4X0";
const POST_B: &str = "3NFG9K0L5QH4";
const POST_C: &str = "A5D6P9V3Q0T";
const POST_D: &str = "C3L7W0F9Q4K8";
const POST_E: &str = "K1P6Q9M2X4J8";
const POST_F: &str = "L3W5N0F8Q2J7";
const POST_G: &str = "M4X1P9L2J6K8";
const POST_H: &str = "N7Q2F5W8J0L3";

const BOOKMARK_ID: &str = "A9G7F2L4Q1W3";

#[tokio::test]
async fn test_post_tag_search() -> Result<()> {
    let post_order = vec![POST_C, POST_B, POST_A, POST_D, POST_E, POST_F];
    let path = format!("{}/{}", ROOT_PATH, OPENSOURCE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 6);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, OPENSOURCE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_limit() -> Result<()> {
    let post_order = vec![POST_C, POST_B];
    let path = format!("{}/{}?limit=2", ROOT_PATH, OPENSOURCE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, OPENSOURCE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_skip() -> Result<()> {
    let post_order = vec![POST_G, POST_H];
    let path = format!("{}/{}?skip=6", ROOT_PATH, OPENSOURCE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, OPENSOURCE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_B];
    let path = format!("{}/{}?skip=1&limit=1", ROOT_PATH, OPENSOURCE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, OPENSOURCE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_viewer_id() -> Result<()> {
    let path = format!("{}/{}?viewer_id={}", ROOT_PATH, OPENSOURCE_LABEL, VIEWER_ID);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 6);

    assert_eq!(tags[0]["bookmark"]["id"], BOOKMARK_ID);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement() -> Result<()> {
    let post_order = vec![POST_A, POST_C, POST_B, POST_G, POST_F, POST_H];
    let path = format!("{}/{}?sorting=totalengagement", ROOT_PATH, OPENSOURCE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 6);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, OPENSOURCE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_limit() -> Result<()> {
    let post_order = vec![POST_A, POST_C];
    let path = format!(
        "{}/{}?sorting=totalengagement&limit=2",
        ROOT_PATH, OPENSOURCE_LABEL
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, OPENSOURCE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_skip() -> Result<()> {
    let post_order = vec![POST_E, POST_D];
    let path = format!(
        "{}/{}?sorting=totalengagement&skip=6",
        ROOT_PATH, OPENSOURCE_LABEL
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, OPENSOURCE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_C];
    let path = format!(
        "{}/{}?sorting=totalengagement&skip=1&limit=1",
        ROOT_PATH, OPENSOURCE_LABEL
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, OPENSOURCE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_specific_tag_with_no_result() -> Result<()> {
    let path = format!("{}/{}", ROOT_PATH, "randommm");
    make_wrong_request(&path).await?;

    Ok(())
}

// TODO: Check if it is in the cache

fn search_tag_in_post(posts: &[Value], label: &str, post_order: Vec<&str>) {
    for (index, post) in posts.iter().enumerate() {
        let mut exist = false;
        // Check if the order of the post is the right one
        assert_eq!(
            post["details"]["id"], post_order[index],
            "The post does not have the right ordering"
        );
        for tag in post["tags"].as_array().unwrap() {
            if tag["label"] == label {
                exist = true;
                break;
            }
        }
        assert!(exist, "The tag was not found in the post. Wrong search")
    }
}
