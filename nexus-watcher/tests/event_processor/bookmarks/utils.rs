use anyhow::Result;
use nexus_common::db::kv::SortOrder;
use nexus_common::db::{fetch_key_from_graph, queries};
use nexus_common::models::post::{Bookmark, PostStream};

pub async fn find_post_bookmark(
    author: &str,
    post_id: &str,
    bookmarker_id: &str,
) -> Result<Bookmark> {
    let query = queries::get::post_bookmark(author, post_id, bookmarker_id);

    let maybe_bookmark = fetch_key_from_graph(query, "b").await.unwrap();

    if let Some(result) = maybe_bookmark {
        return Ok(result);
    }
    anyhow::bail!("Bookmarked relationship not found in Nexus graph");
}

/// The keys of the posts `user_id` bookmarked, newest bookmark first.
pub async fn bookmarked_post_keys(user_id: &str) -> Result<Vec<String>> {
    let stream =
        PostStream::get_bookmarked_posts(user_id, SortOrder::Descending, None, None, None, None)
            .await?;
    Ok(stream.post_keys)
}
