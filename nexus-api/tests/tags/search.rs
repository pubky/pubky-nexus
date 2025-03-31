use anyhow::Result;
use axum::http::StatusCode;
use serde_json::Value;

use crate::{
    stream::post::TAG_LABEL_2,
    utils::{get_request, invalid_get_request},
};

const ROOT_PATH: &str = "/v0/search/tags";
const FREE_LABEL: &str = "free";

const POST_A: &str = "2VDW8YBDZJ02";
const POST_B: &str = "1TDV7XBCF4M1";
const POST_C: &str = "HC3T5CEPBPHQ";

#[tokio_shared_rt::test(shared)]
async fn test_tag_search_by_timeline() -> Result<()> {
    let post_order = vec![POST_A, POST_B, POST_C];
    let path = format!("{}/{}", ROOT_PATH, FREE_LABEL);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 3);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_tag_search_with_skip() -> Result<()> {
    let post_order = vec![POST_B, POST_C];
    let path = format!("{}/{}?skip=1", ROOT_PATH, FREE_LABEL);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_tag_search_with_limit() -> Result<()> {
    let post_order = vec![POST_A];
    let path = format!("{}/{}?limit=1", ROOT_PATH, FREE_LABEL);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_tag_search_with_limit_and_skip() -> Result<()> {
    let post_order = vec![POST_C];
    let path = format!("{}/{}?limit=1&skip=2", ROOT_PATH, FREE_LABEL);
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag_with_no_result() -> Result<()> {
    let path = format!("{}/{}", ROOT_PATH, "randommm");
    invalid_get_request(&path, StatusCode::NOT_FOUND).await?;

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

#[tokio_shared_rt::test(shared)]
async fn test_tag_search_skip_beyond_range() -> Result<()> {
    // Search opensource tag
    let path = format!("{}/{}", ROOT_PATH, TAG_LABEL_2);

    let body = get_request(&path).await?;
    let length = body.as_array().expect("Post list should be an array").len();

    assert!(body.is_array());

    let path_w_skip = format!("{}/{}?skip={}", ROOT_PATH, TAG_LABEL_2, length);
    invalid_get_request(&path_w_skip, StatusCode::NO_CONTENT).await?;

    Ok(())
}
