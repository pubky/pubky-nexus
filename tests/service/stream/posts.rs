use crate::service::utils::{make_request, make_wrong_request};
use anyhow::Result;
use pubky_nexus::{models::post::PostStream, routes::v0::endpoints};
use serde_json::Value;

const ROOT_PATH: &str = endpoints::STREAM_POSTS_ROUTE;

// Ar
const USER_ID: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
// User with most bookmarks
const BOOKMARKED_ID: &str = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";
// Peter user from test/tags.cypher
const VIEWER_ID: &str = "f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a";

const TAG_LABEL_1: &str = "bitcoin";
const TAG_LABEL_2: &str = "opensource";

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
async fn test_stream_posts_global_timeline() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=timeline");
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let mut previous_indexed_at = None;
    for post in body.as_array().expect("Post stream should be an array") {
        let indexed_at = post["details"]["indexed_at"]
            .as_u64()
            .expect("indexed_at should be a valid number");
        if let Some(prev) = previous_indexed_at {
            assert!(indexed_at <= prev, "Posts are not sorted by timeline");
        }
        previous_indexed_at = Some(indexed_at);
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_global_total_engagement() -> Result<()> {
    let path = format!("{ROOT_PATH}?sorting=totalengagement");
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let mut previous_engagement = None;
    for post in body.as_array().expect("Post stream should be an array") {
        let tags = post["counts"]["tags"]
            .as_u64()
            .expect("tags count should be a number");
        let replies = post["counts"]["replies"]
            .as_u64()
            .expect("replies count should be a number");
        let reposts = post["counts"]["reposts"]
            .as_u64()
            .expect("reposts count should be a number");
        let total_engagement = tags + replies + reposts;

        if let Some(prev) = previous_engagement {
            assert!(
                total_engagement <= prev,
                "Posts are not sorted by total engagement"
            );
        }
        previous_engagement = Some(total_engagement);
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_user_posts() -> Result<()> {
    let path = format!("{ROOT_PATH}?author_id={}&sorting=timeline", USER_ID);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert_eq!(
            post["details"]["author"].as_str(),
            Some(USER_ID),
            "Post author should match the requested user"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_following() -> Result<()> {
    let viewer_id = USER_ID;
    let path = format!("{ROOT_PATH}?viewer_id={}&reach=following", viewer_id);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_followers() -> Result<()> {
    let viewer_id = USER_ID;
    let path = format!("{ROOT_PATH}?viewer_id={}&reach=followers", viewer_id);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_bookmarked_posts() -> Result<()> {
    let viewer_id = BOOKMARKED_ID;
    let path = format!("{ROOT_PATH}?viewer_id={}&reach=bookmarks", viewer_id);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_by_tag() -> Result<()> {
    let path = format!("{ROOT_PATH}?tag={}&sorting=timeline", TAG_LABEL_1);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    // Deserialize the response body into a PostStream object
    let post_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(!post_stream.0.is_empty(), "Post stream should not be empty");

    // Iterate over each post and check if it contains the requested tag
    for post in post_stream.0 {
        let has_tag = post.tags.iter().any(|tag| tag.label == TAG_LABEL_1);

        assert!(
            has_tag,
            "Post should be tagged with the requested tag: {}",
            TAG_LABEL_1
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_stream_combined_parameters() -> Result<()> {
    // This one should hit the graph
    let viewer_id = USER_ID;
    let tag = TAG_LABEL_1;
    let path = format!(
        "{ROOT_PATH}?viewer_id={}&reach=following&tag={}&sorting=totalengagement",
        viewer_id, tag
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        let post_tags = post["tags"].as_array().expect("Post should have tags");

        assert!(
            post_tags.iter().any(|t| t.as_str() == Some(tag)),
            "Post should be tagged with the requested tag"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_reach_without_viewer_id() -> Result<()> {
    // Missing viewer_id for a reach query should fail
    let path = format!("{ROOT_PATH}?reach=following");
    make_wrong_request(&path, Some(400)).await?;

    Ok(())
}

#[tokio::test]
async fn test_stream_invalid_sorting() -> Result<()> {
    // Invalid sorting option should fail
    let endpoint = "/v0/stream/posts?sorting=invalid";
    make_wrong_request(endpoint, Some(400)).await?;

    Ok(())
}

#[tokio::test]
async fn test_stream_invalid_reach() -> Result<()> {
    // Invalid reach value should fail
    let path = format!("{ROOT_PATH}?reach=invalid_reach");
    make_wrong_request(&path, Some(400)).await?;

    Ok(())
}

#[tokio::test]
async fn test_stream_bookmarks_without_viewer_id() -> Result<()> {
    // Missing viewer_id for bookmark reach should fail
    let path = format!("{ROOT_PATH}?reach=bookmarks");
    make_wrong_request(&path, Some(400)).await?;

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search() -> Result<()> {
    let post_order = vec![POST_C, POST_B, POST_A, POST_D, POST_E, POST_F];
    let path = format!("{}?tag={}", ROOT_PATH, TAG_LABEL_2);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 6);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_limit() -> Result<()> {
    let post_order = vec![POST_C, POST_B];
    let path = format!("{}?tag={}&limit=2", ROOT_PATH, TAG_LABEL_2);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_skip() -> Result<()> {
    let post_order = vec![POST_G, POST_H];
    let path = format!("{}?tag={}&skip=6", ROOT_PATH, TAG_LABEL_2);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_B];
    let path = format!("{}?tag={}&skip=1&limit=1", ROOT_PATH, TAG_LABEL_2);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_with_viewer_id() -> Result<()> {
    let path = format!("{}?tag={}&viewer_id={}", ROOT_PATH, TAG_LABEL_2, VIEWER_ID);
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
    let path = format!("{}?tag={}&sorting=totalengagement", ROOT_PATH, TAG_LABEL_2);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 6);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_limit() -> Result<()> {
    let post_order = vec![POST_A, POST_C];
    let path = format!(
        "{}?tag={}&sorting=totalengagement&limit=2",
        ROOT_PATH, TAG_LABEL_2
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_skip() -> Result<()> {
    let post_order = vec![POST_E, POST_D];
    let path = format!(
        "{}?tag={}&sorting=totalengagement&skip=6",
        ROOT_PATH, TAG_LABEL_2
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_tag_search_by_engagement_with_skip_and_limit() -> Result<()> {
    let post_order = vec![POST_C];
    let path = format!(
        "{}?tag={}&sorting=totalengagement&skip=1&limit=1",
        ROOT_PATH, TAG_LABEL_2
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_tag_in_post(tags, TAG_LABEL_2, post_order);

    Ok(())
}

#[tokio::test]
async fn test_post_specific_tag_with_no_result() -> Result<()> {
    let path = format!("{}?tag={}", ROOT_PATH, "randommm");
    make_wrong_request(&path, None).await?;

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
