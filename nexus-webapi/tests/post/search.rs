use anyhow::Result;
use nexus_webapi::routes::v0::endpoints::SEARCH_POSTS_BY_TAG_ROUTE;
use serde_json::Value;

use crate::{stream::post::TAG_LABEL_2, utils::get_request};

const POST_A: &str = "2VDW8YBDZJ02";
const POST_B: &str = "1TDV7XBCF4M1";
const POST_C: &str = "HC3T5CEPBPHQ";

pub fn format_search_posts_by_tag(tag: &str) -> String {
    SEARCH_POSTS_BY_TAG_ROUTE.replace("{tag}", tag)
}

fn search_posts_by_tag_free() -> String {
    format_search_posts_by_tag("free")
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_by_timeline() -> Result<()> {
    let post_order = vec![POST_A, POST_B, POST_C];
    let path = search_posts_by_tag_free();
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream posts should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 3);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_with_skip() -> Result<()> {
    let post_order = vec![POST_B, POST_C];
    let path = format!("{}?skip=1", search_posts_by_tag_free());
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream posts should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_with_limit() -> Result<()> {
    let post_order = vec![POST_A];
    let path = format!("{}?limit=1", search_posts_by_tag_free());
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let posts = body.as_array().expect("Stream posts should be an array");

    // Check the total posts using that tag
    assert_eq!(posts.len(), 1);

    // Validate that each post has the searched tag
    search_posts(posts, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_with_limit_and_skip() -> Result<()> {
    let post_order = vec![POST_C];
    let path = format!("{}?limit=1&skip=2", search_posts_by_tag_free());
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream posts should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag_with_no_result() -> Result<()> {
    let path = format_search_posts_by_tag("randommm");
    let body = get_request(&path).await?;

    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

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
async fn test_post_search_skip_beyond_range() -> Result<()> {
    // Search opensource tag
    let path = format_search_posts_by_tag(TAG_LABEL_2);

    let body = get_request(&path).await?;
    let length = body.as_array().expect("Post list should be an array").len();

    assert!(body.is_array());

    let path_w_skip = format!(
        "{}?skip={}",
        format_search_posts_by_tag(TAG_LABEL_2),
        length
    );
    let body = get_request(&path_w_skip).await?;

    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

    Ok(())
}
