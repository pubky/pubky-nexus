use anyhow::Result;
use deadpool_redis::redis::{self, AsyncCommands};
use nexus_common::db::kv::SortOrder;
use nexus_common::db::{fetch_key_from_graph, get_redis_conn, queries};
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

/// The Redis keys a bookmark was copied to before bookmarks were served from
/// the graph: its JSON and the bookmarker's sorted set.
fn retired_keys(author_id: &str, post_id: &str, bookmarker_id: &str) -> (String, String) {
    (
        format!("Bookmark:{author_id}:{post_id}:{bookmarker_id}"),
        format!("Sorted:Bookmarks:User:{bookmarker_id}"),
    )
}

pub async fn assert_no_retired_keys(
    author_id: &str,
    post_id: &str,
    bookmarker_id: &str,
) -> Result<()> {
    let (json_key, set_key) = retired_keys(author_id, post_id, bookmarker_id);
    let mut redis_conn = get_redis_conn().await?;
    for key in [json_key, set_key] {
        let exists: bool = redis_conn.exists(&key).await?;
        assert!(!exists, "{key} must not be written");
    }
    Ok(())
}

/// Writes the retired copies of a bookmark as an older deployment left them,
/// with a score that would put the post first in the stream.
pub async fn seed_retired_keys(
    author_id: &str,
    post_id: &str,
    bookmarker_id: &str,
    bookmark_id: &str,
) -> Result<()> {
    let (json_key, set_key) = retired_keys(author_id, post_id, bookmarker_id);
    let indexed_at: i64 = 9_999_999_999_999;
    let json = serde_json::json!({ "id": bookmark_id, "indexed_at": indexed_at }).to_string();
    let mut redis_conn = get_redis_conn().await?;
    redis::cmd("JSON.SET")
        .arg(&json_key)
        .arg("$")
        .arg(json)
        .query_async::<()>(&mut redis_conn)
        .await?;
    let _: () = redis_conn
        .zadd(&set_key, format!("{author_id}:{post_id}"), indexed_at)
        .await?;
    Ok(())
}

/// The retired copies as they are now: the JSON and the post's score in the set.
pub async fn read_retired_keys(
    author_id: &str,
    post_id: &str,
    bookmarker_id: &str,
) -> Result<(Option<String>, Option<f64>)> {
    let (json_key, set_key) = retired_keys(author_id, post_id, bookmarker_id);
    let mut redis_conn = get_redis_conn().await?;
    let json: Option<String> = redis::cmd("JSON.GET")
        .arg(&json_key)
        .query_async(&mut redis_conn)
        .await?;
    let score: Option<f64> = redis_conn
        .zscore(&set_key, format!("{author_id}:{post_id}"))
        .await?;
    Ok((json, score))
}

pub async fn delete_retired_keys(
    author_id: &str,
    post_id: &str,
    bookmarker_id: &str,
) -> Result<()> {
    let (json_key, set_key) = retired_keys(author_id, post_id, bookmarker_id);
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis_conn.del(&json_key).await?;
    let _: () = redis_conn
        .zrem(&set_key, format!("{author_id}:{post_id}"))
        .await?;
    Ok(())
}
