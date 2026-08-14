use crate::db::get_redis_conn;
use crate::db::graph::exec::fetch_all_rows_from_graph;
use crate::db::graph::Query;
use crate::models::follow::{Followers, Following, UserFollows};
use crate::models::post::create_post_content_index;
use crate::models::post::search::PostsByTagSearch;
use crate::models::post::Bookmark;
use crate::models::tag::post::TagPost;
use crate::models::tag::search::TagSearch;
use crate::models::tag::stream::HotTags;
use crate::models::tag::traits::TagCollection;
use crate::models::tag::user::TagUser;
use crate::models::traits::Collection;
use crate::models::user::{Influencers, UserDetails};
use crate::types::DynError;
use crate::{
    models::post::{PostCounts, PostDetails, PostRelationships},
    models::user::UserCounts,
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{info, Instrument};

/// Clean rebuild of the whole Redis index from the graph: flushes every key,
/// recreates the RediSearch post-content FT index (which the flush drops, and
/// without which full-text post search comes back empty), then reindexes all
/// entities via [`sync`].
pub async fn rebuild() {
    info!("Dropping Redis database...");
    let mut redis_conn = get_redis_conn()
        .await
        .expect("Could not get the redis connection");
    // ASYNC: freeing a production-sized index takes longer than the client
    // response timeout; the keyspace swap itself is synchronous, so the
    // rebuild below writes into the fresh keyspace either way
    deadpool_redis::redis::cmd("FLUSHALL")
        .arg("ASYNC")
        .exec_async(&mut redis_conn)
        .await
        .expect("Failed to flush Redis");

    create_post_content_index()
        .await
        .expect("Failed to create post content FT index");

    info!("Starting reindexing process...");
    sync().await;
}

/// Upper bound on concurrently running entity reindex tasks. Unbounded spawning
/// works on mock-sized datasets but a production graph fans out into thousands
/// of simultaneous Cypher queries and exhausts memory on both ends.
const REINDEX_CONCURRENCY: usize = 32;

#[tracing::instrument(name = "reindex.sync", skip_all)]
pub async fn sync() {
    let mut user_tasks = JoinSet::new();
    let mut post_tasks = JoinSet::new();
    let semaphore = Arc::new(Semaphore::new(REINDEX_CONCURRENCY));

    let user_ids: Vec<String> = get_all_user_ids().await.expect("Failed to get user IDs");
    let user_ids_refs: Vec<&str> = user_ids.iter().map(|id| id.as_str()).collect();

    UserDetails::reindex(&user_ids_refs)
        .await
        .expect("Failed indexing User Details");
    //TODO use collections for every other model

    for user_id in user_ids {
        // Acquire before spawning so pending work queues here instead of as
        // parked tasks; live tasks (and their spans) stay capped at the bound
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed");
        let span = tracing::info_span!("reindex.user", user_id = %user_id);
        user_tasks.spawn(
            async move {
                let _permit = permit;
                if let Err(e) = reindex_user(&user_id).await {
                    tracing::error!("Failed to reindex user {}: {:?}", user_id, e);
                }
            }
            .instrument(span),
        );
    }

    let post_ids = get_all_post_ids().await.expect("Failed to get post IDs");
    for (author_id, post_id) in post_ids {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore closed");
        let span = tracing::info_span!("reindex.post", author_id = %author_id, post_id = %post_id);
        post_tasks.spawn(
            async move {
                let _permit = permit;
                if let Err(e) = reindex_post(&author_id, &post_id).await {
                    tracing::error!("Failed to reindex post {}: {:?}", post_id, e);
                }
            }
            .instrument(span),
        );
    }

    while let Some(res) = user_tasks.join_next().await {
        if let Err(e) = res {
            tracing::error!("User reindexing task failed: {:?}", e);
        }
    }

    while let Some(res) = post_tasks.join_next().await {
        if let Err(e) = res {
            tracing::error!("Post reindexing task failed: {:?}", e);
        }
    }

    HotTags::reindex()
        .await
        .expect("Failed to store the global hot tags");

    Influencers::reindex()
        .await
        .expect("Failed to reindex influencers");

    PostsByTagSearch::reindex()
        .await
        .expect("Failed to store the global post tags");

    TagSearch::reindex()
        .await
        .expect("Failed to store the global tags");

    info!("Reindexing completed successfully.");
}

pub async fn reindex_user(user_id: &str) -> Result<(), DynError> {
    tokio::try_join!(
        Bookmark::reindex(user_id),
        UserCounts::reindex(user_id),
        Followers::reindex(user_id),
        Following::reindex(user_id),
        TagUser::reindex(user_id, None)
    )?;
    Ok(())
}

pub async fn reindex_post(author_id: &str, post_id: &str) -> Result<(), DynError> {
    tokio::try_join!(
        PostDetails::reindex(author_id, post_id),
        PostCounts::reindex(author_id, post_id),
        PostRelationships::reindex(author_id, post_id),
        TagPost::reindex(author_id, Some(post_id))
    )?;
    Ok(())
}

pub async fn get_all_user_ids() -> Result<Vec<String>, DynError> {
    let query = Query::new("get_all_user_ids", "MATCH (u:User) RETURN u.id AS id");
    let rows = fetch_all_rows_from_graph(query).await?;

    let mut user_ids = Vec::new();
    for row in rows {
        if let Some(id) = row.get("id")? {
            user_ids.push(id);
        }
    }

    Ok(user_ids)
}

async fn get_all_post_ids() -> Result<Vec<(String, String)>, DynError> {
    let query = Query::new(
        "get_all_post_ids",
        "MATCH (u:User)-[:AUTHORED]->(p:Post) RETURN u.id AS author_id, p.id AS post_id",
    );
    let rows = fetch_all_rows_from_graph(query).await?;

    let mut post_ids = Vec::new();
    for row in rows {
        if let (Some(author_id), Some(post_id)) = (row.get("author_id")?, row.get("post_id")?) {
            post_ids.push((author_id, post_id));
        }
    }

    Ok(post_ids)
}
