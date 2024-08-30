use anyhow::Result;

use super::utils::{analyse_tag_details_structure, compare_tag_details, make_request, TagMockup};

const PEER_PUBKY: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const POST_ID: &str = "2Z9P8AN738C00";

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
    let hot_tag = TagMockup::new(String::from("pubky"), 1, 1);
    compare_tag_details(&tags[0], hot_tag);

    Ok(())
}

// TODO: Try with other post that has more tags
// TODO: Check if it is in the cache
