use anyhow::Result;

use super::utils::{analyse_tag_details_structure, compare_tag_details, make_request, TagMockup};

// Peter user from test/tags.cypher
const PEER_PUBKY: &str = "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy";
const POST_ID: &str = "0RDV7ABDZDW0";

// TODO: Create deterministic integration tests

#[tokio::test]
async fn test_post_tag() -> Result<()> {
    let path = format!("/v0/post/{}/{}/tags", PEER_PUBKY, POST_ID);
    let body = make_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_tag_details_structure(tags);

    // // Analyse the tag that is in the 4th index
    let hot_tag = TagMockup::new(String::from("🔥"), 4, 4);
    compare_tag_details(&tags[0], hot_tag);

    Ok(())
}

// TODO: Try with other post that has more tags
// TODO: Check if it is in the cache
