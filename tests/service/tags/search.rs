use anyhow::Result;
use serde_json::Value;

use crate::service::utils::{make_request, make_wrong_request};

const ROOT_PATH: &str = "/v0/search/tags";
const FREE_LABEL: &str = "free";

const POST_A: &str = "2VDW8YBDZJ02";
const POST_B: &str = "1TDV7XBCF4M1";
const POST_C: &str = "HC3T5CEPBPHQ";

#[tokio::test]
async fn test_tag_search_by_timeline() -> Result<()> {
    let post_order = vec![POST_A, POST_B, POST_C];
    let path = format!("{}/{}", ROOT_PATH, FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 3);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio::test]
async fn test_tag_search_with_skip() -> Result<()> {
    let post_order = vec![POST_B, POST_C];
    let path = format!("{}/{}?skip=1", ROOT_PATH, FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio::test]
async fn test_tag_search_with_limit() -> Result<()> {
    let post_order = vec![POST_A];
    let path = format!("{}/{}?limit=1", ROOT_PATH, FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio::test]
async fn test_tag_search_with_limit_and_skip() -> Result<()> {
    let post_order = vec![POST_C];
    let path = format!("{}/{}?limit=1&skip=2", ROOT_PATH, FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_specific_tag_with_no_result() -> Result<()> {
    let path = format!("{}/{}", ROOT_PATH, "randommm");
    make_wrong_request(&path, None).await?;

    Ok(())
}

fn search_posts(posts: &[Value], post_order: Vec<&str>) {
    for (index, post) in posts.iter().enumerate() {
        let post_parts: Vec<&str> = post["post_key"].as_str().unwrap().split(':').collect();
        // Check if the order of the post is the right one
        assert_eq!(
            post_parts[1], post_order[index],
            "The post does not have the right ordering"
        );
    }
}
