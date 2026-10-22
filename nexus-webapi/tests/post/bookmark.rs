use crate::post::ROOT_PATH;
use crate::utils::get_request;
use anyhow::Result;

// Bookmarked the author's post over 15 edges, from 1724134095000 to 1724134109000
const VIEWER_ID: &str = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";
const AUTHOR_ID: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const POST_ID: &str = "2Z9P8AN738C00";

/// A post bookmarked over several edges is served the latest one.
#[tokio_shared_rt::test(shared)]
async fn test_post_bookmark_serves_the_latest_of_repeated_edges() -> Result<()> {
    let body = get_request(&format!(
        "{ROOT_PATH}/{AUTHOR_ID}/{POST_ID}/bookmark?viewer_id={VIEWER_ID}"
    ))
    .await?;
    assert_eq!(body["id"], "2Z9ZGRZTJZJ00");
    assert_eq!(body["indexed_at"].as_u64(), Some(1724134109000));

    Ok(())
}
