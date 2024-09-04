use anyhow::Result;
use serde_json::Value;

use super::utils::make_request;

const FREE_LABEL: &str = "free";
// Peter user from test/tags.cypher
const VIEWER_PUBKY: &str = "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy";

const POST_A: &str = "2VDW8YBDZJ02";
const POST_B: &str = "1TDV7XBCF4M1";
const POST_C: &str = "HC3T5CEPBPHQ";

const BOOKMARK_ID: &str = "2Z9PFGC3WWWT0";

#[tokio::test]
async fn test_post_tag_search() -> Result<()> {
    let post_order = vec![POST_A, POST_B, POST_C];
    let path = format!("/v0/search/tags/{}", FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 3);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, FREE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_limit() -> Result<()> {
    let post_order = vec![POST_A, POST_B];
    let path = format!("/v0/search/tags/{}?limit=2", FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, FREE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_skip() -> Result<()> {
    let post_order = vec![POST_C];
    let path = format!("/v0/search/tags/{}?skip=2", FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, FREE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_B];
    let path = format!("/v0/search/tags/{}?skip=1&limit=1", FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, FREE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_viewer_id() -> Result<()> {
    let path = format!("/v0/search/tags/{}?viewer_id={}", FREE_LABEL, VIEWER_PUBKY);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 3);

    assert_eq!(tags[0]["bookmark"]["id"], BOOKMARK_ID);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement() -> Result<()> {
    let post_order = vec![POST_A, POST_C, POST_B];
    let path = format!("/v0/search/tags/{}?sorting=totalengagement", FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 3);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, FREE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_limit() -> Result<()> {
    let post_order = vec![POST_A, POST_C];
    let path = format!("/v0/search/tags/{}?sorting=totalengagement&limit=2", FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, FREE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_skip() -> Result<()> {
    let post_order = vec![POST_B];
    let path = format!("/v0/search/tags/{}?sorting=totalengagement&skip=2", FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, FREE_LABEL, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_C];
    let path = format!("/v0/search/tags/{}?sorting=totalengagement&skip=1&limit=1", FREE_LABEL);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, FREE_LABEL, post_order);

    Ok(())
}

// TODO: Check if it is in the cache

fn search_tag_in_post(posts: &Vec<Value>, label: &str, post_order: Vec<&str>) {
    for (index, post) in posts.iter().enumerate() {
        let mut exist = false;
        // Check if the order of the post is the right one
        assert_eq!(post["details"]["id"], post_order[index], "The post does not have the right ordering");
        for tag in post["tags"].as_array().unwrap() {
            if tag["label"] == label {
                exist = true;
                break;
            }
        }
        assert_eq!(true, exist, "The tag was not found in the post. Wrong search")
    }
}
