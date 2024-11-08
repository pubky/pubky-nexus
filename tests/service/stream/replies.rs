use crate::service::utils::make_request;
use anyhow::Result;
use pubky_nexus::models::post::{PostStream, PostView};

use super::ROOT_PATH;

// Amsterdam user from test/posts.cypher
const AUTHOR_ID: &str = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";
const PARENT_POST_ID: &str = "1A1P4D8C9K0F";

const CHILD_1_POST_ID: &str = "2B9XKZG3T4L6";
const CHILD_2_POST_ID: &str = "3M6WQ8F5P9R2";
const CHILD_3_POST_ID: &str = "4T7ZV0C8K5B1";
const CHILD_4_POST_ID: &str = "5F8YQJ1L2D3E";
const CHILD_5_POST_ID: &str = "6G3ZB9X0H7M4";
const CHILD_6_POST_ID: &str = "7N8K0Y1C3T2Q";

#[tokio::test]
async fn test_stream_posts_replies() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=post_replies&author_id={}&post_id={}",
        AUTHOR_ID, PARENT_POST_ID
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());
    // Deserialize the response body into a PostStream object
    let post_reply_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(
        !post_reply_stream.0.is_empty(),
        "Post stream should not be empty"
    );
    // Assert the post number
    assert_eq!(post_reply_stream.0.len(), 6);

    let replies_order = vec![
        CHILD_6_POST_ID,
        CHILD_5_POST_ID,
        CHILD_4_POST_ID,
        CHILD_3_POST_ID,
        CHILD_2_POST_ID,
        CHILD_1_POST_ID,
    ];

    check_replies_timeline(post_reply_stream.0, replies_order);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_replies_with_limit() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=post_replies&author_id={}&post_id={}&limit=3",
        AUTHOR_ID, PARENT_POST_ID
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());
    // Deserialize the response body into a PostStream object
    let post_reply_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(
        !post_reply_stream.0.is_empty(),
        "Post stream should not be empty"
    );
    // Assert the post number
    assert_eq!(post_reply_stream.0.len(), 3);

    let replies_order = vec![CHILD_6_POST_ID, CHILD_5_POST_ID, CHILD_4_POST_ID];

    check_replies_timeline(post_reply_stream.0, replies_order);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_replies_with_start_query() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=post_replies&author_id={}&post_id={}&start=1719477230025",
        AUTHOR_ID, PARENT_POST_ID
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());
    // Deserialize the response body into a PostStream object
    let post_reply_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(
        !post_reply_stream.0.is_empty(),
        "Post stream should not be empty"
    );
    // Assert the post number
    assert_eq!(post_reply_stream.0.len(), 2);

    let replies_order = vec![CHILD_2_POST_ID, CHILD_1_POST_ID];

    check_replies_timeline(post_reply_stream.0, replies_order);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_replies_with_end_query() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=post_replies&author_id={}&post_id={}&end=1719477230060",
        AUTHOR_ID, PARENT_POST_ID
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());
    // Deserialize the response body into a PostStream object
    let post_reply_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(
        !post_reply_stream.0.is_empty(),
        "Post stream should not be empty"
    );
    // Assert the post number
    assert_eq!(post_reply_stream.0.len(), 3);

    let replies_order = vec![CHILD_6_POST_ID, CHILD_5_POST_ID, CHILD_4_POST_ID];

    check_replies_timeline(post_reply_stream.0, replies_order);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_replies_with_start_and_end_query() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=post_replies&author_id={}&post_id={}&start=1719477230150&end=1719477230017",
        AUTHOR_ID, PARENT_POST_ID
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());
    // Deserialize the response body into a PostStream object
    let post_reply_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(
        !post_reply_stream.0.is_empty(),
        "Post stream should not be empty"
    );
    // Assert the post number
    assert_eq!(post_reply_stream.0.len(), 4);

    let replies_order = vec![
        CHILD_5_POST_ID,
        CHILD_4_POST_ID,
        CHILD_3_POST_ID,
        CHILD_2_POST_ID,
    ];

    check_replies_timeline(post_reply_stream.0, replies_order);

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_replies_with_start_and_end_also_limit_query() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?source=post_replies&author_id={}&post_id={}&start=1719477230150&end=1719477230017&limit=3",
        AUTHOR_ID, PARENT_POST_ID
    );
    let body = make_request(&path).await?;

    assert!(body.is_array());
    // Deserialize the response body into a PostStream object
    let post_reply_stream: PostStream = serde_json::from_value(body)?;

    // Ensure the stream has posts
    assert!(
        !post_reply_stream.0.is_empty(),
        "Post stream should not be empty"
    );
    // Assert the post number
    assert_eq!(post_reply_stream.0.len(), 3);

    let replies_order = vec![CHILD_5_POST_ID, CHILD_4_POST_ID, CHILD_3_POST_ID];

    check_replies_timeline(post_reply_stream.0, replies_order);

    Ok(())
}

pub fn check_replies_timeline(posts: Vec<PostView>, post_order: Vec<&str>) {
    for (index, post) in posts.iter().enumerate() {
        // Check if the order of the post is the right one
        assert_eq!(
            post.details.id, post_order[index],
            "The timeline of post replies is not the right one"
        );
    }
}
