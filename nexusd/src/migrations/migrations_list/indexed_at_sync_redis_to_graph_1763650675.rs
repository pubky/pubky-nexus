use async_trait::async_trait;
use deadpool_redis::redis::AsyncCommands;
use neo4rs::query;
use nexus_common::{
    db::{fetch_key_from_graph, get_redis_conn, RedisOps},
    models::{
        file::FileDetails,
        post::{Bookmark, PostDetails},
        traits::Collection,
        user::UserDetails,
    },
    types::DynError,
};
use tracing::{info, warn};

use crate::migrations::manager::Migration;

pub struct IndexedAtSyncRedisToGraph1763650675;

const USER_DETAILS_PREFIX: &str = "User:Details";
const POST_DETAILS_PREFIX: &str = "Post:Details";
const FILE_DETAILS_PREFIX: &str = "File:Details";
const BOOKMARK_PREFIX: &str = "Bookmark";

#[async_trait]
impl Migration for IndexedAtSyncRedisToGraph1763650675 {
    fn id(&self) -> &'static str {
        "IndexedAtSyncRedisToGraph1763650675"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        let users = sync_user_details_indexed_at().await?;
        let posts = sync_post_details_indexed_at().await?;
        let files = sync_file_details_indexed_at().await?;
        let bookmarks = sync_bookmark_indexed_at().await?;

        info!(
            updated_users = users.1,
            inspected_users = users.0,
            updated_posts = posts.1,
            inspected_posts = posts.0,
            updated_files = files.1,
            inspected_files = files.0,
            updated_bookmarks = bookmarks.1,
            inspected_bookmarks = bookmarks.0,
            "Completed indexed_at synchronization from Redis to Neo4j"
        );

        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}

fn split_key_tail<'a>(key: &'a str, prefix: &str, expected: usize) -> Option<Vec<&'a str>> {
    let remainder = key.strip_prefix(&format!("{prefix}:"))?;
    let parts: Vec<&str> = remainder.split(':').collect();
    if parts.len() != expected {
        warn!(
            key,
            prefix,
            expected,
            actual = parts.len(),
            "Skipping malformed redis key"
        );
        return None;
    }
    Some(parts)
}

async fn sync_user_details_indexed_at() -> Result<(usize, usize), DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let mut scan = redis_conn
        .scan_match::<_, String>(format!("{USER_DETAILS_PREFIX}:*"))
        .await?;

    let mut inspected = 0usize;
    let mut updated = 0usize;

    while let Some(key) = scan.next_item().await {
        inspected += 1;
        let Some(parts) = split_key_tail(&key, USER_DETAILS_PREFIX, 1) else {
            continue;
        };
        let user_id = parts[0];
        let Some(redis_details) = UserDetails::try_from_index_json(&[user_id], None).await? else {
            warn!(
                user_id,
                "Missing UserDetails payload in Redis while syncing indexed_at"
            );
            continue;
        };

        let graph_details = UserDetails::get_from_graph(&[user_id]).await?;
        let Some(Some(graph_details)) = graph_details.first() else {
            warn!(user_id, "User missing from graph while syncing indexed_at");
            continue;
        };

        if graph_details.indexed_at == redis_details.indexed_at {
            continue;
        }

        let matched = fetch_key_from_graph(
            query(
                "MATCH (u:User {id: $user_id})\n                 SET u.indexed_at = $indexed_at\n                 RETURN count(u) AS matched",
            )
            .param("user_id", user_id)
            .param("indexed_at", redis_details.indexed_at),
            "matched",
        )
        .await?
        .unwrap_or(0i64);

        if matched > 0 {
            updated += 1;
        } else {
            warn!(user_id, "No User node matched for indexed_at update");
        }
    }

    Ok((inspected, updated))
}

async fn sync_post_details_indexed_at() -> Result<(usize, usize), DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let mut scan = redis_conn
        .scan_match::<_, String>(format!("{POST_DETAILS_PREFIX}:*"))
        .await?;

    let mut inspected = 0usize;
    let mut updated = 0usize;

    while let Some(key) = scan.next_item().await {
        inspected += 1;
        let Some(parts) = split_key_tail(&key, POST_DETAILS_PREFIX, 2) else {
            continue;
        };
        let author_id = parts[0];
        let post_id = parts[1];

        let Some(redis_post) =
            PostDetails::try_from_index_json(&[author_id, post_id], None).await?
        else {
            warn!(
                author_id,
                post_id, "Missing PostDetails payload in Redis while syncing indexed_at"
            );
            continue;
        };

        let graph_post = PostDetails::get_from_graph(author_id, post_id).await?;
        let Some((graph_post, _)) = graph_post else {
            warn!(
                author_id,
                post_id, "Post missing from graph while syncing indexed_at"
            );
            continue;
        };

        if graph_post.indexed_at == redis_post.indexed_at {
            continue;
        }

        let matched = fetch_key_from_graph(
            query(
                "MATCH (:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})\n                 SET p.indexed_at = $indexed_at\n                 RETURN count(p) AS matched",
            )
            .param("author_id", author_id)
            .param("post_id", post_id)
            .param("indexed_at", redis_post.indexed_at),
            "matched",
        )
        .await?
        .unwrap_or(0i64);

        if matched > 0 {
            updated += 1;
        } else {
            warn!(
                author_id,
                post_id, "No Post node matched for indexed_at update"
            );
        }
    }

    Ok((inspected, updated))
}

async fn sync_file_details_indexed_at() -> Result<(usize, usize), DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let mut scan = redis_conn
        .scan_match::<_, String>(format!("{FILE_DETAILS_PREFIX}:*"))
        .await?;

    let mut inspected = 0usize;
    let mut updated = 0usize;

    while let Some(key) = scan.next_item().await {
        inspected += 1;
        let Some(parts) = split_key_tail(&key, FILE_DETAILS_PREFIX, 2) else {
            continue;
        };
        let owner_id = parts[0];
        let file_id = parts[1];

        let Some(redis_file) = FileDetails::try_from_index_json(&[owner_id, file_id], None).await?
        else {
            warn!(
                owner_id,
                file_id, "Missing FileDetails payload in Redis while syncing indexed_at"
            );
            continue;
        };

        let graph_files = FileDetails::get_from_graph(&[&[owner_id, file_id]]).await?;
        let Some(Some(graph_file)) = graph_files.first() else {
            warn!(
                owner_id,
                file_id, "File missing from graph while syncing indexed_at"
            );
            continue;
        };

        if graph_file.indexed_at == redis_file.indexed_at {
            continue;
        }

        let matched = fetch_key_from_graph(
            query(
                "MATCH (f:File {id: $file_id, owner_id: $owner_id})\n                 SET f.indexed_at = $indexed_at\n                 RETURN count(f) AS matched",
            )
            .param("owner_id", owner_id)
            .param("file_id", file_id)
            .param("indexed_at", redis_file.indexed_at),
            "matched",
        )
        .await?
        .unwrap_or(0i64);

        if matched > 0 {
            updated += 1;
        } else {
            warn!(
                owner_id,
                file_id, "No File node matched for indexed_at update"
            );
        }
    }

    Ok((inspected, updated))
}

async fn sync_bookmark_indexed_at() -> Result<(usize, usize), DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let mut scan = redis_conn
        .scan_match::<_, String>(format!("{BOOKMARK_PREFIX}:*"))
        .await?;

    let mut inspected = 0usize;
    let mut updated = 0usize;

    while let Some(key) = scan.next_item().await {
        inspected += 1;
        let Some(parts) = split_key_tail(&key, BOOKMARK_PREFIX, 3) else {
            continue;
        };
        let author_id = parts[0];
        let post_id = parts[1];
        let viewer_id = parts[2];

        let Some(redis_bookmark) =
            Bookmark::try_from_index_json(&[author_id, post_id, viewer_id], None).await?
        else {
            warn!(
                author_id,
                post_id, viewer_id, "Missing Bookmark payload in Redis while syncing indexed_at"
            );
            continue;
        };

        let graph_bookmark = Bookmark::get_from_graph(author_id, post_id, viewer_id).await?;
        let Some(graph_bookmark) = graph_bookmark else {
            warn!(
                author_id,
                post_id, viewer_id, "Bookmark missing from graph while syncing indexed_at"
            );
            continue;
        };

        if graph_bookmark.indexed_at == redis_bookmark.indexed_at {
            continue;
        }

        let matched = fetch_key_from_graph(
            query(
                "MATCH (:User {id: $author_id})<-[:AUTHORED]-(p:Post {id: $post_id})\n                 MATCH (:User {id: $viewer_id})-[b:BOOKMARKED {id: $bookmark_id}]->(p)\n                 SET b.indexed_at = $indexed_at\n                 RETURN count(b) AS matched",
            )
            .param("author_id", author_id)
            .param("post_id", post_id)
            .param("viewer_id", viewer_id)
            .param("bookmark_id", redis_bookmark.id.as_str())
            .param("indexed_at", redis_bookmark.indexed_at),
            "matched",
        )
        .await?
        .unwrap_or(0i64);

        if matched > 0 {
            updated += 1;
        } else {
            warn!(
                author_id,
                post_id, viewer_id, "No BOOKMARKED relationship matched for indexed_at update"
            );
        }
    }

    Ok((inspected, updated))
}
