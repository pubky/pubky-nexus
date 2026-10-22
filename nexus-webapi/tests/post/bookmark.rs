use crate::post::ROOT_PATH;
use crate::utils::get_request;
use anyhow::Result;

// Bookmarks their own post
const BOOKMARKER_ID: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
const POST_ID: &str = "2ZCW1TGR5BKG0";

/// The point read serves the graph's bookmark.
#[tokio_shared_rt::test(shared)]
async fn test_post_bookmark_is_read_from_the_graph() -> Result<()> {
    let body = get_request(&format!(
        "{ROOT_PATH}/{BOOKMARKER_ID}/{POST_ID}/bookmark?viewer_id={BOOKMARKER_ID}"
    ))
    .await?;
    assert_eq!(body["id"], "2Z9PFGC3WWWW0");
    assert_eq!(body["indexed_at"].as_u64(), Some(1721764200000));

    Ok(())
}
