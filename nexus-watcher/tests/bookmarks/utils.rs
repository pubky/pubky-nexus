use anyhow::Result;
use nexus_common::db::{queries, retrieve_from_graph};
use nexus_common::models::post::Bookmark;

pub async fn find_post_bookmark(
    author: &str,
    post_id: &str,
    bookmarker_id: &str,
) -> Result<Bookmark> {
    let query = queries::get::post_bookmark(author, post_id, bookmarker_id);

    let maybe_bookmark = retrieve_from_graph(query, "b").await.unwrap();

    if let Some(result) = maybe_bookmark {
        return Ok(result);
    }
    anyhow::bail!("Bookmarked relationship not found in Nexus graph");
}
